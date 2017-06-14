#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate error_chain;

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate jsonrpc_core;
extern crate jsonrpc_pubsub;
#[macro_use]
extern crate jsonrpc_macros;
extern crate jsonrpc_ws_server;
extern crate uuid;
#[macro_use]
extern crate lazy_static;

extern crate talpid_core;
extern crate talpid_ipc;

mod management_interface;
mod states;

use management_interface::{ManagementInterfaceServer, TunnelCommand};
use states::{SecurityState, TargetState};

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use talpid_core::net::RemoteAddr;
use talpid_core::tunnel::{self, TunnelEvent, TunnelMonitor};

error_chain!{
    errors {
        /// The client is in the wrong state for the requested operation. Optimally the code should
        /// be written in such a way so such states can't exist.
        InvalidState {
            description("Client is in an invalid state for the requested operation")
        }
        TunnelError(msg: &'static str) {
            description("Error in the tunnel monitor")
            display("Tunnel monitor error: {}", msg)
        }
        ManagementInterfaceError(msg: &'static str) {
            description("Error in the management interface")
            display("Management interface error: {}", msg)
        }
    }
}

lazy_static! {
    // Temporary store of hardcoded remotes.
    static ref REMOTES: [RemoteAddr; 3] = [
        RemoteAddr::new("se5.mullvad.net", 1300),
        RemoteAddr::new("se6.mullvad.net", 1300),
        RemoteAddr::new("se7.mullvad.net", 1300),
    ];
}

pub enum DaemonEvent {
    TunnelEvent(TunnelEvent),
    TunnelExit(tunnel::Result<()>),
    ManagementInterfaceEvent(TunnelCommand),
    ManagementInterfaceExit(talpid_ipc::Result<()>),
}

impl From<TunnelEvent> for DaemonEvent {
    fn from(tunnel_event: TunnelEvent) -> Self {
        DaemonEvent::TunnelEvent(tunnel_event)
    }
}

impl From<TunnelCommand> for DaemonEvent {
    fn from(tunnel_command: TunnelCommand) -> Self {
        DaemonEvent::ManagementInterfaceEvent(tunnel_command)
    }
}

/// Represents the internal state of the actual tunnel.
#[derive(Debug, Eq, PartialEq)]
pub enum TunnelState {
    /// No tunnel is running.
    NotRunning,
    /// The tunnel has been started, but it is not established/functional.
    Down,
    /// The tunnel is up and working.
    Up,
}

impl TunnelState {
    pub fn as_security_state(&self) -> SecurityState {
        match *self {
            TunnelState::Up => SecurityState::Secured,
            _ => SecurityState::Unsecured,
        }
    }
}


struct Daemon {
    state: TunnelState,
    last_broadcasted_state: SecurityState,
    target_state: TargetState,
    rx: mpsc::Receiver<DaemonEvent>,
    tx: mpsc::Sender<DaemonEvent>,
    tunnel_close_handle: Option<tunnel::CloseHandle>,
    management_interface_subscribers: management_interface::EventBroadcaster,

    // Just for testing. A cyclic iterator iterating over the hardcoded remotes,
    // picking a new one for each retry.
    remote_iter: std::iter::Cycle<std::iter::Cloned<std::slice::Iter<'static, RemoteAddr>>>,
}

impl Daemon {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let management_interface_subscribers = Self::start_management_interface(tx.clone())?;
        Ok(
            Daemon {
                state: TunnelState::NotRunning,
                last_broadcasted_state: SecurityState::Unsecured,
                target_state: TargetState::Unsecured,
                rx,
                tx,
                tunnel_close_handle: None,
                remote_iter: REMOTES.iter().cloned().cycle(),
                management_interface_subscribers,
            },
        )
    }

    // Starts the management interface and spawns a thread that will process it.
    // Returns a handle that allows notifying all subscribers on events.
    fn start_management_interface(event_tx: mpsc::Sender<DaemonEvent>)
                                  -> Result<management_interface::EventBroadcaster> {
        let server = Self::start_management_interface_server(event_tx.clone())?;
        let event_broadcaster = server.event_broadcaster();
        Self::spawn_management_interface_wait_thread(server, event_tx);
        Ok(event_broadcaster)
    }

    fn start_management_interface_server(event_tx: mpsc::Sender<DaemonEvent>)
                                         -> Result<ManagementInterfaceServer> {
        let server =
            ManagementInterfaceServer::start(event_tx.clone())
                .chain_err(|| ErrorKind::ManagementInterfaceError("Failed to start server"),)?;
        info!(
            "Mullvad management interface listening on {}",
            server.address()
        );
        Ok(server)
    }

    fn spawn_management_interface_wait_thread(server: ManagementInterfaceServer,
                                              exit_tx: mpsc::Sender<DaemonEvent>) {
        thread::spawn(
            move || {
                let result = server.wait();
                debug!("Mullvad management interface shut down");
                let _ = exit_tx.send(DaemonEvent::ManagementInterfaceExit(result));
            },
        );
    }

    /// Consume the `Daemon` and run the main event loop. Blocks until an error happens.
    pub fn run(mut self) -> Result<()> {
        while let Ok(event) = self.rx.recv() {
            self.handle_event(event)?;
        }
        Ok(())
    }

    fn handle_event(&mut self, event: DaemonEvent) -> Result<()> {
        use DaemonEvent::*;
        match event {
            TunnelEvent(event) => Ok(self.handle_tunnel_event(event)),
            TunnelExit(result) => self.handle_tunnel_exit(result),
            ManagementInterfaceEvent(event) => self.handle_management_interface_event(event),
            ManagementInterfaceExit(result) => self.handle_management_interface_exit(result),
        }
    }

    fn handle_tunnel_event(&mut self, tunnel_event: TunnelEvent) {
        info!("Tunnel event: {:?}", tunnel_event);
        let new_state = match tunnel_event {
            TunnelEvent::Up => TunnelState::Up,
            TunnelEvent::Down => TunnelState::Down,
        };
        self.set_state(new_state);
    }

    fn handle_tunnel_exit(&mut self, result: tunnel::Result<()>) -> Result<()> {
        self.tunnel_close_handle = None;
        if let Err(e) = result {
            log_error("Tunnel exited in an unexpected way", e);
        }
        self.set_state(TunnelState::NotRunning);
        if self.target_state == TargetState::Secured {
            self.start_tunnel()?;
        }
        Ok(())
    }

    fn handle_management_interface_event(&mut self, event: TunnelCommand) -> Result<()> {
        match event {
            TunnelCommand::SetTargetState(state) => self.set_target_state(state)?,
            TunnelCommand::GetState(tx) => {
                if let Err(_) = tx.send(self.last_broadcasted_state) {
                    warn!("Unable to send current state to management interface client",);
                }
            }
        }
        Ok(())
    }

    fn handle_management_interface_exit(&self, result: talpid_ipc::Result<()>) -> Result<()> {
        let error = ErrorKind::ManagementInterfaceError("Server exited unexpectedly");
        match result {
            Ok(()) => Err(error.into()),
            e => e.chain_err(|| error),
        }
    }

    /// Set the target state of the client. If it changed trigger the operations needed to progress
    /// towards that state.
    fn set_target_state(&mut self, new_state: TargetState) -> Result<()> {
        if new_state != self.target_state {
            self.target_state = new_state;
            match self.target_state {
                TargetState::Secured => {
                    if self.state == TunnelState::NotRunning {
                        debug!("Triggering tunnel start from management interface event");
                        self.start_tunnel()?;
                    }
                }
                TargetState::Unsecured => {
                    if let Some(close_handle) = self.tunnel_close_handle.take() {
                        debug!("Triggering tunnel stop from management interface event");
                        // This close operation will block until the tunnel is dead.
                        close_handle
                            .close()
                            .chain_err(|| ErrorKind::TunnelError("Unable to kill tunnel"))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Update the state of the client. If it changed, notify the subscribers.
    fn set_state(&mut self, new_state: TunnelState) {
        if new_state != self.state {
            self.state = new_state;
            let new_security_state = self.state.as_security_state();
            if self.last_broadcasted_state != new_security_state {
                self.last_broadcasted_state = new_security_state;
                self.management_interface_subscribers.notify_new_state(new_security_state);
            }
        }
    }

    fn start_tunnel(&mut self) -> Result<()> {
        ensure!(
            self.state == TunnelState::NotRunning,
            ErrorKind::InvalidState
        );
        let remote = self.remote_iter.next().unwrap();
        let tunnel_monitor = self.spawn_tunnel_monitor(remote)?;
        self.tunnel_close_handle = Some(tunnel_monitor.close_handle());
        self.spawn_tunnel_monitor_wait_thread(tunnel_monitor);

        self.set_state(TunnelState::Down);
        Ok(())
    }

    fn spawn_tunnel_monitor(&self, remote: RemoteAddr) -> Result<TunnelMonitor> {
        // Must wrap the channel in a Mutex because TunnelMonitor forces the closure to be Sync
        let event_tx = Arc::new(Mutex::new(self.tx.clone()));
        let on_tunnel_event = move |event| {
            let _ = event_tx.lock().unwrap().send(DaemonEvent::TunnelEvent(event));
        };
        TunnelMonitor::new(remote, on_tunnel_event)
            .chain_err(|| ErrorKind::TunnelError("Unable to start tunnel monitor"))
    }

    fn spawn_tunnel_monitor_wait_thread(&self, tunnel_monitor: TunnelMonitor) {
        let error_tx = self.tx.clone();
        thread::spawn(
            move || {
                let result = tunnel_monitor.wait();
                let _ = error_tx.send(DaemonEvent::TunnelExit(result));
                trace!("Tunnel monitor thread exit");
            },
        );
    }
}


fn log_error<E>(msg: &str, error: E)
    where E: error_chain::ChainedError
{
    error!("{}: {}", msg, error);
    for e in error.iter().skip(1) {
        error!("Caused by {}", e);
    }
}


quick_main!(run);

fn run() -> Result<()> {
    init_logger()?;

    let daemon = Daemon::new().chain_err(|| "Unable to initialize daemon")?;
    daemon.run()?;

    debug!("Mullvad daemon is quitting");
    Ok(())
}

fn init_logger() -> Result<()> {
    env_logger::init().chain_err(|| "Failed to bootstrap logging system")
}
