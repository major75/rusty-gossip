use super::common::{now, NetworkState, SharedNetworkState};
use super::sync::sync_state;

use futures::prelude::*;
use settimeout::set_timeout;
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

type ReceivedStates = HashMap<String, Option<NetworkState>>;

const BEAT_DURATION_MSEC: u64 = 100;
const HEART_BEAT_DURATION_MSEC: u64 = BEAT_DURATION_MSEC * 10;

pub async fn start_heartbeat(period: u8, state: SharedNetworkState, alive_duration: u64) {
    // Create beat counter
    let mut ticks = 0;

    let period: u64 = period as u64 * 1000;
    let mut connected = false;

    loop {
        // Will send message to the network if there are connected peers to send the message to
        if connected && (((ticks * BEAT_DURATION_MSEC) % period) == 0) {
            let msg = format!("Time: {}", now());
            broadcast(state.clone(), Some(msg), alive_duration).await;
        } else if ((ticks * BEAT_DURATION_MSEC) % HEART_BEAT_DURATION_MSEC) == 0 {
            // log::debug!("Client. Will broadcast heartbeat");

            // Broadcast heartbeat alive message about self to the network
            broadcast(state.clone(), None, alive_duration).await;
        }

        // Output connected
        if !connected {
            let initial_network_state: NetworkState = match state.lock() {
                Ok(v) => v.clone(),
                Err(e) => {
                    log::error!("Failed to acquire broadcast lock. Error: {}", e);
                    return;
                }
            };

            // Populate foreign peers list
            let mut dest_list = "".to_owned();
            let mut sep = "".to_owned();
            for dest_peer in &initial_network_state.peers {
                if dest_peer.id != initial_network_state.sender {
                    dest_list = format!("{}{}\"{}\"", dest_list, sep, dest_peer.id);
                    sep = ", ".to_owned();
                    connected = true;
                }
            }

            if connected {
                // Print connected message
                log::info!("Connected to the peers at [{}]", dest_list);
            }
        }

        set_timeout(Duration::from_millis(BEAT_DURATION_MSEC)).await;
        ticks += 1;
    }
}

async fn broadcast(state: SharedNetworkState, payload: Option<String>, alive_duration: u64) {
    let mut my_network_state: NetworkState = match state.lock() {
        Ok(v) => v.clone(),
        Err(e) => {
            log::error!("Failed to acquire broadcast lock. Error: {}", e);
            return;
        }
    };

    if !(my_network_state.peers.len() > 1) {
        // Do not broadcast if the are no peers can connect to
        return;
    }

    // Prepare sending message destination peers list
    let mut dest_list = "".to_owned();
    let mut sep = "".to_owned();
    for dest_peer in &my_network_state.peers {
        if dest_peer.id != my_network_state.sender {
            dest_list = format!("{}{}\"{}\"", dest_list, sep, dest_peer.id);
            sep = ", ".to_owned();
        }
    }

    // Update heartbeat of self peer
    if let Some(self_peer) = my_network_state.peers.iter_mut().find(|item| {
        return item.id == my_network_state.sender;
    }) {
        self_peer.heartbeat = now();

        // Also set payload and increment version if we also broadcast payload
        if let Some(_) = payload {
            self_peer.version += 1;
            self_peer.payload = payload;

            if let Some(msg) = &self_peer.payload {
                log::info!("Sending message [{}] to [{}]", msg, dest_list);
            }
        }
    }

    let mut received_states = ReceivedStates::new();

    // TODO implement futures all at once start
    for peer in &my_network_state.peers {
        // Skip self peer
        if peer.id != my_network_state.sender {
            log::debug!("Client. Will heartbeat to: {}. Data: {:?}", peer.id, my_network_state);
            if let Some(received) = send_network_state_to(&peer.id, &my_network_state).await {
                received_states.insert(peer.id.clone(), Some(received));
            } else {
                received_states.insert(peer.id.clone(), None);
            }
        }
    }

    sync_received_states(&received_states, &mut my_network_state, alive_duration, now());

    // Save result state as my shared network state
    {
        let mut result_state = match state.lock() {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to acquire broadcast lock. Error: {}", e);
                return;
            }
        };

        *result_state = my_network_state;
    }
}

async fn send_network_state_to(peer: &str, state: &NetworkState) -> Option<NetworkState> {
    // Connect to server
    if let Ok(socket) = TcpStream::connect(&peer).await {
        // log::debug!("Client. Connected to: {}", socket.peer_addr().unwrap());

        // Delimit frames using a length header
        let length_delimited = Framed::new(socket, LengthDelimitedCodec::new());

        // Serialize frames with JSON
        let mut writer =
            tokio_serde::SymmetricallyFramed::new(length_delimited, SymmetricalJson::default());

        let json = serde_json::to_value(&state).expect("To JSON serialization error");

        // Send the value
        match writer.send(json).await {
            Ok(_) => {
                while let Some(msg) = writer.try_next().await.unwrap() {
                    match serde_json::from_value(msg) {
                        Ok(ret) => {
                            log::debug!(
                                "Client. Got response from peer: {}. Data: {:?}",
                                peer,
                                ret
                            );

                            return Some(ret);
                        }
                        Err(e) => {
                            log::error!("Got unrecognized data from peer: \"{}\". Error: {}", peer, e);
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to send network state to peer: \"{}\". Error: {}", peer, e);
            }
        }
    } else {
        log::warn!("Failed to connect to: \"{}\"", peer);
    }

    return None;
}

fn sync_received_states(
    foreign_states: &ReceivedStates,
    recipient_state: &mut NetworkState,
    alive_duration: u64,
    now: u64,
) {
    // Sync states
    for item in foreign_states {
        if let (_, Some(peer_state)) = item {
            sync_state(peer_state, recipient_state, alive_duration, now);
        }
    }

    // Delete from result empty state -> not responsive peers
    for item in foreign_states {
        if let (delete_peer_id, None) = item {
            recipient_state.peers.retain_mut(|item| {
                if item.id == *delete_peer_id {
                    return false;
                }
                return true;
            });
        }
    }
}

#[cfg(test)]
mod test {
    use super::super::NetworkState;
    use super::super::PeerState;
    use super::{sync_received_states, ReceivedStates};

    #[test]
    fn test_sync_received_states() {
        let foreign_state_peer2 = NetworkState {
            sender: "peer2".to_owned(),
            peers: vec![
                PeerState {
                    id: "peer1".to_owned(),
                    version: 1,
                    heartbeat: 1,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer2".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer3".to_owned(),
                    version: 3,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer4".to_owned(),
                    version: 4,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer5".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer6".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
            ],
        };

        let foreign_state_peer4 = NetworkState {
            sender: "peer4".to_owned(),
            peers: vec![
                PeerState {
                    id: "peer1".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer2".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer3".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer4".to_owned(),
                    version: 4,
                    heartbeat: 7,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer5".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer6".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
            ],
        };

        let foreign_state_peer5 = NetworkState {
            sender: "peer5".to_owned(),
            peers: vec![
                PeerState {
                    id: "peer1".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer2".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer3".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer4".to_owned(),
                    version: 4,
                    heartbeat: 7,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer5".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "peer6".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
            ],
        };

        let mut foreign_states: ReceivedStates = ReceivedStates::new();
        foreign_states.insert("peer2".to_owned(), Some(foreign_state_peer2));
        foreign_states.insert("peer3".to_owned(), None);
        foreign_states.insert("peer4".to_owned(), Some(foreign_state_peer4));
        foreign_states.insert("peer5".to_owned(), Some(foreign_state_peer5));
        foreign_states.insert("peer6".to_owned(), None);

        let mut recipient_state = NetworkState {
            sender: "peer1".to_owned(),
            peers: vec![PeerState {
                id: "peer1".to_owned(),
                version: 1,
                heartbeat: 1,
                payload: None,
                updated: None,
            }],
        };

        sync_received_states(&foreign_states, &mut recipient_state, 5, 11);
        println!("Recipient state: {:?}", recipient_state);
        assert_eq!(recipient_state.peers.len(), 4);
    }
}
