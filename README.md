# dirsync

Dirsync is a command-line tool written in Rust for syncing directories between a client and a server, which contains client and server functions in the binary simultaneously.
This tool is for rust study. There are three branches with different implementations, but they share the same command-line usage.

- tcpstream-bincode
  
  use `std::net::TcpStream` and depends on `bincode` crate which is used to serialize and deserialize rust structs. The release binary size is 647KB which is the smallest of the three branches.
- tokio-async-bincode
  
  depends on `tokio` and `bincode` crates. The release binary size is about 775KB.
- tokio-http
  
  depends on `tokio` and `actix-web` crates. The server provides service as rest api through a http server. The release binary size is 4.09MB, which is the largest among all.

## build

optional: `git checkout tcpstream-bincode` or `git checkout tokio-async-bincode` or `git checkout tokio-http`

`cargo build --release`

## usage
### launch server

`dirsync server -l :9022 -d /path/to/base/dir`

### server options:
- -l: listen address
- -d: directory to serve
- --auth-key: authorization key [optional]

### launch client to sync from server

`dirsync sync -v --dry-run -s :9022 -d /path/to/client/dir`

### client options:
- -s: server address
- -d: directory to sync
- -v: for debug output
- --auth-key: authorization key [optional, should be the same with server's auth-key]
- --dry-run: just check which files will be updated
