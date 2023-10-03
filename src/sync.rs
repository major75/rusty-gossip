use super::NetworkState;

pub fn sync_state(
    foreign_state: &NetworkState,
    recipient_state: &mut NetworkState,
    alive_duration: u64,
    now: u64
) {
    // Process all foreign peers that exist in foreign or both in foreign and recipient
    for fi in &foreign_state.peers {
        // Find this peer in target state
        match recipient_state.peers.iter_mut().find(|ti| {
            return fi.id == ti.id;
        }) {
            Some(ri) => {
                // Peer from the foreign state was found in the target state

                // Sync recipient state
                if foreign_state.sender == ri.id {
                    // Peer is the sender
                    // Forcibly set sender's peer to alive state
                    ri.heartbeat = now;

                    if fi.version > ri.version {
                        ri.version = fi.version;
                        ri.payload = fi.payload.clone();
                        ri.updated = Some(true);

                        // Process payload if needed
                        if let Some(msg) = &ri.payload {
                            let out = format!("Received message [{}] from \"{}\" ", &msg, &ri.id);
                            log::info!("{}", &out);
                        }
                    }
                } else if fi.version > ri.version {
                    // Ensure that foreign peer is really alive.
                    // And is not the one we have lost connection to.
                    // Then both its heartbeat and version will be greater then the peer's instance from local state
                    if fi.heartbeat > ri.heartbeat {
                        ri.version = fi.version;
                        ri.heartbeat = fi.heartbeat;
                        ri.payload = fi.payload.clone();
                        ri.updated = Some(true);

                        // Process payload if needed
                        if let Some(msg) = &ri.payload {
                            let out = format!("Received message [{}] from \"{}\" ", &msg, &ri.id);
                            log::info!("{}", &out);
                        }
                    }
                } else if fi.version == ri.version {
                    // Update heartbeat
                    if fi.heartbeat > ri.heartbeat {
                        ri.heartbeat = fi.heartbeat;
                    }
                }
            }
            None => {
                // Peer from the foreign state was not found in the target state

                if fi.id == foreign_state.sender {
                    // Add foreign peer to the target state
                    let mut new_peer = fi.clone();
                    new_peer.updated = Some(true);

                    // Process payload if needed
                    if let Some(msg) = &new_peer.payload {
                        let out = format!("Received message [{}] from \"{}\" ", &msg, &new_peer.id);
                        log::info!("{}", &out);
                    }

                    // Add new peer to the state
                    recipient_state.peers.push(new_peer);
                } else if fi.heartbeat + alive_duration >= now {
                    // For other peers add them with initial state.
                    // Those peers states will be synced and updated later on after the heartbeat

                    let mut new_peer = fi.clone();
                    new_peer.version = 0;
                    new_peer.payload = None;
                    new_peer.updated = Some(true);

                    // Add new peer to the state
                    recipient_state.peers.push(new_peer);
                }
            }
        }
    }

    // Process all peers that exist only in recipient state and not in the foreign one
    recipient_state.peers.retain_mut(|item| {
        // Update self peer state to retain it in the state
        if item.id == recipient_state.sender {
            item.heartbeat = now;
            item.updated = Some(true);
        }

        // Retain in the state only alive items
        if let Some(updated) = item.updated {
            if updated == false {
                if item.heartbeat + alive_duration >= now {
                    return true;
                }
                return false;
            }
            return true;
        } else {
            if item.heartbeat + alive_duration >= now {
                return true;
            }
            return false;
        }
    });

    // Delete updated flag
    for item in &mut recipient_state.peers {
        item.updated = None;
    }
}

#[cfg(test)]
mod test {
    use super::super::PeerState;
    use super::*;

    #[test]
    fn test_sync_init() {
        let foreign_state = NetworkState {
            sender: "sender".to_owned(),
            peers: vec![PeerState { // Peer is sender
                id: "sender".to_owned(),
                version: 1,
                heartbeat: 10,
                payload: Some("Sender's message".to_owned()),
                updated: None,
            }],
        };

        let mut recipient_state = NetworkState {
            sender: "recipient".to_owned(),
            peers: vec![PeerState {
                id: "recipient".to_owned(),
                version: 2,
                heartbeat: 1,
                payload: Some("Recepient's message".to_owned()),
                updated: None,
            }],
        };

        sync_state(&foreign_state, &mut recipient_state, 2, 12);
        println!("Recipient state: {:?}", recipient_state);
        assert_eq!(recipient_state.peers.len(), 2);
        assert_eq!(recipient_state.peers[0].id, "recipient");
        assert_eq!(recipient_state.peers[1].id, "sender");
    }

    #[test]
    fn test_sync_full() {
        let foreign_state = NetworkState {
            sender: "sender".to_owned(),
            peers: vec![
                PeerState { // Alive peer
                    id: "peer3".to_owned(),
                    version: 3,
                    heartbeat: 10,
                    payload: Some("Peer3 v3 message".to_owned()),
                    updated: None,
                },
                PeerState { // Sender peer
                    id: "sender".to_owned(),
                    version: 2,
                    heartbeat: 10,
                    payload: Some("Sender's v2 message".to_owned()),
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer4".to_owned(),
                    version: 4,
                    heartbeat: 10,
                    payload: Some("Peer4 v4 message".to_owned()),
                    updated: None,
                },
                PeerState { // Recipient peer
                    id: "recipient".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Dead peer
                    id: "peer5".to_owned(),
                    version: 5,
                    heartbeat: 8,
                    payload: Some("Peer5 v5 message".to_owned()),
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer6".to_owned(),
                    version: 3,
                    heartbeat: 10,
                    payload: Some("Peer6 v3 message".to_owned()),
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer10".to_owned(),
                    version: 3,
                    heartbeat: 8,
                    payload: Some("Peer10 v3 message".to_owned()),
                    updated: None,
                },
            ],
        };

        let mut recipient_state = NetworkState {
            sender: "recipient".to_owned(),
            peers: vec![
                PeerState {
                    id: "recipient".to_owned(),
                    version: 1,
                    heartbeat: 1,
                    payload: None,
                    updated: None,
                },
                PeerState { // Dead peer
                    id: "peer5".to_owned(),
                    version: 5,
                    heartbeat: 8,
                    payload: Some("Peer5 v5 message".to_owned()),
                    updated: None,
                },
                PeerState {
                    id: "sender".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer3".to_owned(),
                    version: 2,
                    heartbeat: 9,
                    payload: Some("Peer3 v2 message".to_owned()),
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer8".to_owned(),
                    version: 8,
                    heartbeat: 10,
                    payload: Some("Peer8 v8 message".to_owned()),
                    updated: None,
                },
                PeerState { // Dead peer
                    id: "peer9".to_owned(),
                    version: 8,
                    heartbeat: 8,
                    payload: Some("Peer9 v8 message".to_owned()),
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer10".to_owned(),
                    version: 4,
                    heartbeat: 10,
                    payload: Some("Peer10 v4 message".to_owned()),
                    updated: None,
                },
            ],
        };

        sync_state(&foreign_state, &mut recipient_state, 2, 11);
        println!("Recipient state: {:?}", recipient_state);
        assert_eq!(recipient_state.peers.len(), 7);
    }

    #[test]
    fn test_sync_add_from_foreign() {
        let foreign_state = NetworkState {
            sender: "sender".to_owned(),
            peers: vec![
                PeerState { // Peer is sender
                    id: "sender".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer3".to_owned(),
                    version: 4,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Dead peer
                    id: "peer4".to_owned(),
                    version: 4,
                    heartbeat: 8,
                    payload: None,
                    updated: None,
                },
            ],
        };

        let mut recipient_state = NetworkState {
            sender: "recipient".to_owned(),
            peers: vec![PeerState {
                id: "recipient".to_owned(),
                version: 1,
                heartbeat: 1,
                payload: None,
                updated: None,
            }],
        };

        sync_state(&foreign_state, &mut recipient_state, 2, 12);
        println!("Recipient state: {:?}", recipient_state);
        assert_eq!(recipient_state.peers.len(), 3);
        assert_eq!(recipient_state.peers[2].id, "peer3");
    }

    #[test]
    fn test_sync_in_favour_of_foreign() {
        let foreign_state = NetworkState {
            sender: "sender".to_owned(),
            peers: vec![
                PeerState { // Recipient peer
                    id: "recipient".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Sender peer
                    id: "sender".to_owned(),
                    version: 2,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer3".to_owned(),
                    version: 3,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
            ],
        };

        let mut recipient_state = NetworkState {
            sender: "recipient".to_owned(),
            peers: vec![
                PeerState {
                    id: "recipient".to_owned(),
                    version: 1,
                    heartbeat: 1,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "sender".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
            ],
        };

        sync_state(&foreign_state, &mut recipient_state, 2, 11);
        println!("Recipient state: {:?}", recipient_state);
        assert_eq!(recipient_state.peers.len(), 3);
        assert_eq!(recipient_state.peers[2].version, 0);
    }

    #[test]
    fn test_sync_delete_not_updated() {
        let foreign_state = NetworkState {
            sender: "sender".to_owned(),
            peers: vec![
                PeerState { // Recipient peer
                    id: "recipient".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Sender peer
                    id: "sender".to_owned(),
                    version: 2,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
            ],
        };

        let mut recipient_state = NetworkState {
            sender: "recipient".to_owned(),
            peers: vec![
                PeerState {
                    id: "recipient".to_owned(),
                    version: 1,
                    heartbeat: 1,
                    payload: None,
                    updated: None,
                },
                PeerState {
                    id: "sender".to_owned(),
                    version: 1,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer3".to_owned(),
                    version: 3,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Dead peer
                    id: "peer4".to_owned(),
                    version: 4,
                    heartbeat: 7,
                    payload: None,
                    updated: None,
                },
                PeerState { // Alive peer
                    id: "peer5".to_owned(),
                    version: 5,
                    heartbeat: 10,
                    payload: None,
                    updated: None,
                },
                PeerState { // Dead peer
                    id: "peer6".to_owned(),
                    version: 5,
                    heartbeat: 8,
                    payload: None,
                    updated: None,
                },
            ],
        };

        sync_state(&foreign_state, &mut recipient_state, 2, 11);
        println!("Recipient state: {:?}", recipient_state);
        assert_eq!(recipient_state.peers.len(), 4);
        assert_eq!(recipient_state.peers[2].id, "peer3");
        assert_eq!(recipient_state.peers[3].id, "peer5");
    }
}
