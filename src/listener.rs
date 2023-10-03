use super::common::{now, NetworkState, SharedNetworkState};
use super::sync::sync_state;

use futures::prelude::*;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_serde::formats::*;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub async fn start_listener(listener: TcpListener, state: SharedNetworkState, alive_duration: u64) {
    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let foreign_peer =
                    socket.peer_addr().expect("No peer address obtained from incoming connection");
                log::debug!("Server. Got incoming connection from peer: {}", foreign_peer);

                // Delimit frames using a length header
                let length_delimited = Framed::new(socket, LengthDelimitedCodec::new());

                // Deserialize frames
                let mut reader = tokio_serde::SymmetricallyFramed::new(
                    length_delimited,
                    SymmetricalJson::<Value>::default(),
                );

                // Spawn a task that prints all received messages to STDOUT
                let state = state.clone();
                tokio::spawn(async move {
                    while let Some(msg) = match reader.try_next().await {
                        Ok(v) => v,
                        Err(e) => {
                            log::error!("Error reading network state request from socket. Sending peer: {}. Error: {}", foreign_peer, e);
                            return;
                        }
                    } {
                        let foreign_peer: String = format!("{}", foreign_peer);
                        log::debug!("Server. Got request from peer: {}. Data: {}", foreign_peer, msg);

                        let got_state: NetworkState = match serde_json::from_value(msg) {
                            Ok(v) => v,
                            Err(e) => {
                                log::error!("Error parsing network state request. Sending peer: {}. Error: {}", foreign_peer, e);
                                return;
                            }
                        };

                        log::debug!("Server. Before sync state is. Data: {:?}", &*state);

                        {
                            let mut my_network_state = match state.lock() {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!("Failed to acquire broadcast lock. Error: {}", e);
                                    return;
                                }
                            };

                            // Sync incoming connection peer's state with the local state
                            sync_state(&got_state, &mut my_network_state, alive_duration, now());
                        }

                        // Send response to the client peer
                        let json = serde_json::to_value(&*state)
                            .expect("Network state should be serializable to JSON");
                        reader.send(json).await.unwrap();

                        log::debug!("Server. After sync state is. Data: {:?}", &*state);
                    }
                });
            }
            Err(e) => {
                log::error!("{}", e);
                break;
            }
        }
    }
}
