mod common;
mod heartbeat;
mod listener;
mod sync;

use common::NetworkState;
use common::PeerState;

use clap::Parser;
use heartbeat as mh;
use listener as ml;
use log::LevelFilter;

use dotenv::dotenv;
use fern::colors::{Color, ColoredLevelConfig};
use std::env;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::task;

const PEER_ALIVE_DURATION_SEC: u64 = 2;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Optional. String in the format: <address>:<port>. Address of the network seed node peer should connect to.
    /// If omitted the peer considered to be a seed node.
    #[arg(long)]
    connect: Option<String>,

    /// Number. Send message interval in seconds
    #[arg(long)]
    period: Option<u8>,

    /// Number. Listening port, a number in range 1024 - 65535, typically 80xx
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
pub async fn main() {
    dotenv().ok();

    set_up_logging().expect("Failed to setup application's logger");

    let args = Args::parse();

    let port = if let Some(port) = args.port {
        port.to_string()
    } else {
        let port = env::var("port").expect("Listening port must be set");
        port
    };

    let local_address = format!("127.0.0.1:{}", port);

    // Bind a server socket
    let listener = match TcpListener::bind(&local_address).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to start listening on address: \"{}\". Error: {}", &local_address, e);
            return;
        }
    };

    let local_addr = listener
        .local_addr()
        .expect("No local address obtained from server listening connection");
    let local_addr = format!("{}", local_addr);

    log::info!("My address is: \"{}\"", local_address);

    // Network initial state
    let mut state = NetworkState {
        sender: local_addr.clone(),
        peers: vec![PeerState {
            id: local_addr.to_owned(),
            version: 0,
            heartbeat: 0,
            payload: None,
            updated: None,
        }],
    };

    // Set seed node endpoint
    let seed_node = if let Some(connect) = args.connect {
        connect
    } else {
        if let Ok(seed_node) = env::var("connect") {
            seed_node
        } else {
            "".to_owned()
        }
    };

    if seed_node.len() != 0 {
        state.peers.push(PeerState {
            id: seed_node,
            version: 0,
            heartbeat: 0,
            payload: None,
            updated: None,
        });
    }

    let state: Arc<Mutex<NetworkState>> = Arc::new(Mutex::new(state));

    let period = if let Some(period) = args.period {
        period
    } else {
        let period: String = env::var("period").expect("Period must be set");
        period.parse::<u8>().expect("Period parameter is not unsigned integer")
    };

    task::spawn(mh::start_heartbeat(period, state.clone(), PEER_ALIVE_DURATION_SEC));

    task::spawn(ml::start_listener(listener, state.clone(), PEER_ALIVE_DURATION_SEC));

    signal::ctrl_c().await.expect("failed to listen for Ctrl-c signal");

    log::info!("Stopping gossip node. Ctrl-c signal received");
}

fn set_up_logging() -> Result<(), fern::InitError> {
    // configure colors for the whole line
    let palette = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::White) // we actually don't need to specify the color for debug and info, they are white by default
        .trace(Color::BrightBlack); // depending on the terminals color scheme, this is the same as the background color

    // configure colors for the name of the level.
    // since almost all of them are the same as the color for the whole line, we
    // just clone `colors_line` and overwrite our changes
    let colors_level = palette.clone().info(Color::Green);

    let log_filter: LevelFilter = match env::var("log_level").unwrap_or("info".to_owned()).as_str() {
        "debug" => log::LevelFilter::Debug,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info
    };

    // here we set up our fern Dispatch
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{palette}[{date}][{level}{palette}] - {message}\x1B[0m",
                palette = format_args!("\x1B[{}m", palette.get_color(&record.level()).to_fg_str()),
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        .level(log_filter)
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;

    Ok(())
}
