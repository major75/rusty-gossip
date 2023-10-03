# rusty-gossip

An example of simple p2p gossiping network implementation written on Rust lang. Network peers use a kind of heartbeat technique to synchronize network state among each others. Peers will send an arbitrary messages to each other within time period (in seconds) specified as a per peer runtime parameter.

#### Program usage:

```
$ ./rusty-gossip --help
A simple p2p gossip network peer implementation

Usage: rusty-gossip [OPTIONS]

Options:
      --connect <CONNECT>  Optional. String in the format: <address>:<port>. Address of the network seed node peer should connect to. If omitted the peer considered to be a seed node
      --period <PERIOD>    Number. Send message interval in seconds
      --port <PORT>        Number. Listening port, a number in range 1024 - 65535, typically 80xx
  -h, --help               Print help
  -V, --version            Print version
```

Also you can setup program options in .env file placed in the same directory with the executable. Then you will be able to run the program without command line options. Be aware cli options have higher priority over .env file.

#### Examples of usage via command line options:

Seed node:  
`$ ./rusty-gossip --port 8080 --period 5`

Peers:  
`./rusty-gossip --port 8081 --period 5 --connect 127.0.0.1:8080`  

#### An example of .env file

```
# Number. Send message interval in seconds
period=5
# Number. Listening port, a number in range 1024 - 65535, typically 80xx
port=8081
# Optional. String in the format: <address>:<port>. Address of the network seed node peer should connect to.
# If omitted the peer considered to be a seed node.
connect=127.0.0.1:8080
# log verbosity level: debug, info, warn, error. Default is info
log_level=debug
```
