/// Constants for OpenVPN. Taken from include/openvpn-plugin.h in the OpenVPN repository:
/// https://github.com/OpenVPN/openvpn/blob/master/include/openvpn-plugin.h.in

use std::collections::HashMap;
use std::os::raw::c_int;


// All types of events that a plugin can receive from OpenVPN.
pub const OPENVPN_PLUGIN_UP: c_int = 0;
pub const OPENVPN_PLUGIN_DOWN: c_int = 1;
pub const OPENVPN_PLUGIN_ROUTE_UP: c_int = 2;
pub const OPENVPN_PLUGIN_IPCHANGE: c_int = 3;
pub const OPENVPN_PLUGIN_TLS_VERIFY: c_int = 4;
pub const OPENVPN_PLUGIN_AUTH_USER_PASS_VERIFY: c_int = 5;
pub const OPENVPN_PLUGIN_CLIENT_CONNECT: c_int = 6;
pub const OPENVPN_PLUGIN_CLIENT_DISCONNECT: c_int = 7;
pub const OPENVPN_PLUGIN_LEARN_ADDRESS: c_int = 8;
pub const OPENVPN_PLUGIN_CLIENT_CONNECT_V2: c_int = 9;
pub const OPENVPN_PLUGIN_TLS_FINAL: c_int = 10;
pub const OPENVPN_PLUGIN_ENABLE_PF: c_int = 11;
pub const OPENVPN_PLUGIN_ROUTE_PREDOWN: c_int = 12;
pub const OPENVPN_PLUGIN_N: c_int = 13;

lazy_static! {
    pub static ref PLUGIN_EVENT_NAMES: HashMap<c_int, &'static str> = {
        let mut map = HashMap::new();
        map.insert(OPENVPN_PLUGIN_UP, "PLUGIN_UP");
        map.insert(OPENVPN_PLUGIN_DOWN, "PLUGIN_DOWN");
        map.insert(OPENVPN_PLUGIN_ROUTE_UP, "PLUGIN_ROUTE_UP");
        map.insert(OPENVPN_PLUGIN_IPCHANGE, "PLUGIN_IPCHANGE");
        map.insert(OPENVPN_PLUGIN_TLS_VERIFY, "PLUGIN_TLS_VERIFY");
        map.insert(OPENVPN_PLUGIN_AUTH_USER_PASS_VERIFY, "PLUGIN_AUTH_USER_PASS_VERIFY");
        map.insert(OPENVPN_PLUGIN_CLIENT_CONNECT, "PLUGIN_CLIENT_CONNECT");
        map.insert(OPENVPN_PLUGIN_CLIENT_DISCONNECT, "PLUGIN_CLIENT_DISCONNECT");
        map.insert(OPENVPN_PLUGIN_LEARN_ADDRESS, "PLUGIN_LEARN_ADDRESS");
        map.insert(OPENVPN_PLUGIN_CLIENT_CONNECT_V2, "PLUGIN_CLIENT_CONNECT_V2");
        map.insert(OPENVPN_PLUGIN_TLS_FINAL, "PLUGIN_TLS_FINAL");
        map.insert(OPENVPN_PLUGIN_ENABLE_PF, "PLUGIN_ENABLE_PF");
        map.insert(OPENVPN_PLUGIN_ROUTE_PREDOWN, "PLUGIN_ROUTE_PREDOWN");
        map.insert(OPENVPN_PLUGIN_N, "PLUGIN_N");
        map
    };
}

/// Returns the name of an OPENVPN_PLUGIN_* constant.
pub fn plugin_event_name(num: c_int) -> &'static str {
    PLUGIN_EVENT_NAMES.get(&num).map(|s| *s).unwrap_or("UNKNOWN")
}


// Return values. Returned from the plugin to OpenVPN to indicate success or failure. Can also
// Accept (success) or decline (error) an operation, such as incoming client connection attempt.
pub const OPENVPN_PLUGIN_FUNC_SUCCESS: c_int = 0;
pub const OPENVPN_PLUGIN_FUNC_ERROR: c_int = 1;
pub const OPENVPN_PLUGIN_FUNC_DEFERRED: c_int = 2;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_event_name_up() {
        let name = plugin_event_name(0);
        assert_eq!("PLUGIN_UP", name);
    }

    #[test]
    fn plugin_event_name_n() {
        let name = plugin_event_name(13);
        assert_eq!("PLUGIN_N", name);
    }

    #[test]
    fn plugin_event_name_not_existing() {
        let name = plugin_event_name(-15);
        assert_eq!("UNKNOWN", name);
    }
}