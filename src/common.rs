use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerState {
    pub id: String,
    pub version: u64,
    pub heartbeat: u64,
    pub payload: Option<String>,
    pub updated: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkState {
    pub sender: String,
    pub peers: Vec<PeerState>,
}

pub type SharedNetworkState = Arc<Mutex<NetworkState>>;

pub fn now() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    return since_the_epoch.as_secs();
}

#[cfg(test)]
mod test {

    use super::now;

    #[test]
    fn test_now() {
        let n = now();
        println!("{}", n);
        assert!(now() > 1696090587);
    }
}
