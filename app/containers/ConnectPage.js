// @flow

import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { push } from 'react-router-redux';
import { links } from '../config';
import Connect from '../components/Connect';
import connectActions from '../redux/connection/actions';
import { openLink } from '../lib/platform';

import type { ReduxState, ReduxDispatch } from '../redux/store';
import type { SharedRouteProps } from '../routes';

import type { RelaySettingsRedux, RelayLocationRedux } from '../redux/settings/reducers';

function getRelayName(relaySettings: RelaySettingsRedux, relayLocations: Array<RelayLocationRedux>): string {
  if(relaySettings.normal) {
    const location = relaySettings.normal.location;

    if(location === 'any') {
      return 'Automatic';
    } else if(location.country) {
      const country = relayLocations.find(({ code }) => code === location.country);
      if(country) {
        return country.name;
      }
    } else if(location.city) {
      const [countryCode, cityCode] = location.city;
      const country = relayLocations.find(({ code }) => code === countryCode);
      if(country) {
        const city = country.cities.find(({ code }) => code === cityCode);
        if(city) {
          return city.name;
        }
      }
    }

    return 'Unknown';
  } else if(relaySettings.custom_tunnel_endpoint) {
    return 'Custom';
  } else {
    throw new Error('Unsupported relay settings.');
  }
}

const mapStateToProps = (state: ReduxState) => {
  return {
    accountExpiry: state.account.expiry,
    selectedRelayName: getRelayName(
      state.settings.relaySettings,
      state.settings.relayLocations
    ),
    connection: state.connection,
  };
};

const mapDispatchToProps = (dispatch: ReduxDispatch, props: SharedRouteProps) => {
  const { connect, disconnect, copyIPAddress } = bindActionCreators(connectActions, dispatch);
  const { push: pushHistory } = bindActionCreators({ push }, dispatch);
  const { backend } = props;

  return {
    onSettings: () => {
      pushHistory('/settings');
    },
    onSelectLocation: () => {
      pushHistory('/select-location');
    },
    onConnect: () => {
      connect(backend);
    },
    onCopyIP: () => {
      copyIPAddress();
    },
    onDisconnect: () => {
      disconnect(backend);
    },
    onExternalLink: (type) => openLink(links[type]),
  };
};

export default connect(mapStateToProps, mapDispatchToProps)(Connect);
