## Bitcoin Cap'n Proto Rust Client

This project auto-generates the client code to interact with Bitcoin Core in Rust using interprocess communication.

## Build prerequisites

Building this crate requires the [`capnp`](https://capnproto.org/install.html)
compiler.

macOS:

```sh
brew install capnp
```

Debian / Ubuntu:

```sh
sudo apt-get install capnproto libcapnp-dev
```

If `/capnp/c++.capnp` cannot be found during `cargo build`, install the
platform's Cap'n Proto development package in addition to the compiler.

## Minimum Standard Rust Version

To compile this crate your project must use a Rust compiler of **1.85** or higher.

## Building

```sh
cargo build
```

## Running integration tests

The integration tests connect to a running bitcoin node via IPC.

### 1. Build Bitcoin Core

```sh
cd /path/to/bitcoin
cmake -B build -DENABLE_WALLET=OFF -DBUILD_TESTS=OFF
cmake --build build -j$(nproc)
```

### 2. Start bitcoin

```sh
./build/bin/bitcoin node -chain=regtest -ipcbind=unix -server -debug=ipc -daemon
```

### 3. Generate blocks

The mining tests require chain height > 16. At height <= 16, `createNewBlock`
fails with `bad-cb-length` because the BIP34 height push is too short for the
coinbase scriptSig minimum.

```sh
./build/bin/bitcoin rpc -chain=regtest -rpcwait generatetodescriptor 101 "raw(51)"
```

### 4. Run tests

```sh
cargo test
```

### 5. Stop bitcoin

```sh
./build/bin/bitcoin rpc -chain=regtest stop
```

## License

Creative Commons 1.0 Universal
