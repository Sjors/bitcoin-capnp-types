use std::path::{Path, PathBuf};

use bitcoin_capnp_types::{
    init_capnp::init,
    mining_capnp::{block_template, mining},
    proxy_capnp::{thread, thread_map},
};
use capnp_rpc::{RpcSystem, rpc_twoparty_capnp::Side, twoparty::VatNetwork};
use futures::io::BufReader;
use tokio::net::{UnixStream, unix::OwnedReadHalf};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub fn unix_socket_path() -> PathBuf {
    let home_dir_string = std::env::var("HOME").unwrap();
    let home_dir = home_dir_string.parse::<PathBuf>().unwrap();
    let bitcoin_dir = if cfg!(target_os = "macos") {
        home_dir
            .join("Library")
            .join("Application Support")
            .join("Bitcoin")
    } else {
        home_dir.join(".bitcoin")
    };
    let regtest_dir = bitcoin_dir.join("regtest");
    regtest_dir.join("node.sock")
}

pub async fn connect_unix_stream(
    path: impl AsRef<Path>,
) -> VatNetwork<BufReader<Compat<OwnedReadHalf>>> {
    let path = path.as_ref();
    let mut last_err = None;
    for _ in 0..10 {
        match UnixStream::connect(path).await {
            Ok(stream) => {
                let (reader, writer) = stream.into_split();
                let buf_reader = futures::io::BufReader::new(reader.compat());
                let buf_writer = futures::io::BufWriter::new(writer.compat_write());
                return VatNetwork::new(buf_reader, buf_writer, Side::Client, Default::default());
            }
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
    panic!(
        "unix socket connection to {} failed after retries: {}. Is bitcoin running with -ipcbind=unix?",
        path.display(),
        last_err.unwrap()
    );
}

/// Bootstrap an Init client, spawn the RPC system, and create a thread handle.
pub async fn bootstrap(
    mut rpc_system: RpcSystem<capnp_rpc::rpc_twoparty_capnp::Side>,
) -> (init::Client, thread::Client) {
    let client: init::Client = rpc_system.bootstrap(Side::Server);
    tokio::task::spawn_local(rpc_system);
    let create_client_response = client
        .construct_request()
        .send()
        .promise
        .await
        .expect("could not create initial request");
    let thread_map: thread_map::Client = create_client_response
        .get()
        .unwrap()
        .get_thread_map()
        .unwrap();
    let thread_reponse = thread_map
        .make_thread_request()
        .send()
        .promise
        .await
        .unwrap();
    let thread: thread::Client = thread_reponse.get().unwrap().get_result().unwrap();
    (client, thread)
}

/// Obtain a Mining client from an Init client.
pub async fn make_mining(init: &init::Client, thread: &thread::Client) -> mining::Client {
    let mut req = init.make_mining_request();
    req.get().get_context().unwrap().set_thread(thread.clone());
    let resp = req.send().promise.await.unwrap();
    resp.get().unwrap().get_result().unwrap()
}

/// Create a new block template with default options and no cooldown.
///
/// The node must have height > 16. At height <= 16 the BIP34 height push
/// is only one byte, which is shorter than the two-byte minimum scriptSig
/// required by consensus (see `CheckTransaction`), causing `createNewBlock`
/// to fail with `bad-cb-length`. Either generate blocks via bitcoin rpc
/// (`generatetodescriptor`) before running these tests, or (in a real miner)
/// pad the coinbase scriptSig with an extra push like `OP_0`.
pub async fn make_block_template(
    mining: &mining::Client,
    thread: &thread::Client,
) -> block_template::Client {
    let mut req = mining.create_new_block_request();
    req.get().get_context().unwrap().set_thread(thread.clone());
    req.get().set_cooldown(false);
    let resp = req.send().promise.await.unwrap();
    resp.get().unwrap().get_result().unwrap()
}

/// Destroy a block template.
pub async fn destroy_template(template: &block_template::Client, thread: &thread::Client) {
    let mut req = template.destroy_request();
    req.get().get_context().unwrap().set_thread(thread.clone());
    req.send().promise.await.unwrap();
}
