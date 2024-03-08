# Kinode

Kinode OS is a decentralized OS, built for crypto.

This repo contains the core runtime and processes.
Most developers need not build the runtime.
Instead, check out the [Kinode book](https://book.kinode.org/), and in particular the ["My First App" tutorial](https://book.kinode.org/my_first_app/chapter_1.html).

If you want to get on the network, you can download a binary, rather than building it yourself, from [the releases page](https://github.com/kinode-dao/kinode/tags).
Then follow the instructions to [install it](https://book.kinode.org/install.html) and [join the network](https://book.kinode.org/login.html).

If you have questions, join the [Kinode discord](https://discord.gg/TCgdca5Bjt) and drop us a question!

## Setup

### Building components

On certain operating systems, you may need to install these dependencies if they are not already present:
- openssl-sys: https://docs.rs/crate/openssl-sys/0.9.19
- libclang 5.0: https://rust-lang.github.io/rust-bindgen/requirements.html

```bash
# Clone the repo.

git clone git@github.com:kinode-dao/kinode.git

# Get some stuff so we can build Wasm.

cd kinode
cargo install wasm-tools
rustup install nightly
rustup target add wasm32-wasi
rustup target add wasm32-wasi --toolchain nightly
cargo install cargo-wasi

# Build the runtime, along with a number of "distro" WASM modules
# OPTIONAL: --release flag

cargo +nightly build -p kinode
```

### Boot
Get an eth-sepolia-rpc API key and pass that as an argument. You can get one for free at `alchemy.com`.

Make sure not to use the same home directory for two nodes at once! You can use any name for the home directory: here we just use `home`. The `--` here separates cargo arguments from binary arguments.

TODO: document feature flags in `--simulation-mode`
```bash
# OPTIONAL: --release flag
cargo +nightly run -p kinode -- home --testnet
```

On boot you will be prompted to navigate to `localhost:8080`. Make sure your browser wallet matches the network that the node is being booted on. Follow the registration UI -- if you want to register a new ID you will either need [Sepolia testnet tokens](https://www.infura.io/faucet/sepolia) or an invite code.

### Configuring the ETH RPC Provider

By default, a node will use the hardcoded providers for the network ([testnet](./kinode/default_providers_testnet.json)/[mainnet](./kinode/default_providers_mainnet.json)) it is booted on. A node can use a WebSockets RPC URL directly, or use another Kinode as a relay point. To adjust the providers a node uses, just create and modify the `.eth_providers` file in the node's home folder (set at boot). See the Kinode Book for more docs, and see the [default providers file here](./kinode/default_providers_testnet.json) for a template to create `.eth_providers`.

### Distro and Runtime processes

The base OS install comes with certain runtime modules. These are interacted with in the same way as userspace processes, but are deeply ingrained to the system and the APIs they present at their Process IDs are assumed to be available by userspace processes. All of these are identified in the `distro:sys` package.

This distribution of the OS also comes with userspace packages pre-installed. Some of these packages are intimately tied to the runtime: `terminal`, `homepage`, and `kns_indexer`. Modifying, removing or replacing the distro userspace packages should only be done in highly specialized use-cases.

The runtime distro processes are:

- `eth:distro:sys`
- `http_client:distro:sys`
- `http_server:distro:sys`
- `kernel:distro:sys`
- `kv:distro:sys`
- `net:distro:sys`
- `state:distro:sys`
- `terminal:distro:sys`
- `timer:distro:sys`
- `sqlite:distro:sys`
- `vfs:distro:sys`

The distro userspace packages are:

- `app_store:sys`
- `chess:sys`
- `homepage:sys`
- `kns_indexer:sys`
- `terminal:sys`
- `tester:sys` (used with `kit` for running test suites)

The `sys` publisher is not a real node ID, but it's also not a special case value. Packages, whether runtime or userspace, installed from disk when a node bootstraps do not have their package ID or publisher node ID validated. Packages installed (not injected locally, as is done during development) after a node has booted will have their publisher field validated.

### Terminal syntax

- CTRL+C or CTRL+D to gracefully shutdown node
- CTRL+V to toggle through verbose modes (0-3, 0 is default and lowest verbosity)

- CTRL+J to toggle debug mode
- CTRL+S to step through events in debug mode

- CTRL+L to toggle logging mode, which writes all terminal output to the `.terminal_log` file. Off by default, this will write all events and verbose prints with timestamps.

- CTRL+A to jump to beginning of input
- CTRL+E to jump to end of input
- UpArrow/DownArrow or CTRL+P/CTRL+N to move up and down through command history
- CTRL+R to search history, CTRL+R again to toggle through search results, CTRL+G to cancel search

- `m <address> <json>`: send an inter-process message. <address> is formatted as <node>@<process_id>. <process_id> is formatted as <process_name>:<package_name>:<publisher_node>. JSON containing spaces must be wrapped in single-quotes (`''`).
    - Example: `m our@eth:distro:sys "SetPublic" -a 5`
    - the '-a' flag is used to expect a response with a given timeout
    - `our` will always be interpolated by the system as your node's name
- `hi <name> <string>`: send a text message to another node's command line.
    - Example: `hi ben.os hello world`
- `top <process_id>`: display kernel debugging info about a process. Leave the process ID blank to display info about all processes and get the total number of running processes.
    - Example: `top net:distro:sys`
    - Example: `top`
- `cat <vfs-file-path>`: print the contents of a file in the terminal
    - Example: `cat /terminal:sys/pkg/scripts.json`
- `echo <text>`: print `text` to the terminal
    - Example: `echo foo`
- `net_diagnostics`: print some useful networking diagnostic data
- `peers`: print the peers the node currently hold connections with
- `peer <name>`: print the peer's PKI info, if it exists

### Terminal example usage

Download and install an app:
```
m our@main:app_store:sys '{"Download": {"package": {"package_name": "<pkg>", "publisher_node": "<node>"}, "install_from": "<node>"}}'
m our@main:app_store:sys '{"Install": {"package_name": "<pkg>", "publisher_node": "<node>"}}'
```
