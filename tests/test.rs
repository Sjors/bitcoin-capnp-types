use std::path::{Path, PathBuf};

use bitcoin_capnp_types::{
    init_capnp::init,
    proxy_capnp::{thread, thread_map},
};
use capnp_rpc::{RpcSystem, rpc_twoparty_capnp::Side, twoparty::VatNetwork};
use futures::io::BufReader;
use tokio::{
    net::{UnixStream, unix::OwnedReadHalf},
    task::LocalSet,
};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

fn unix_socket_path() -> PathBuf {
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

async fn connect_unix_stream(
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
async fn bootstrap(
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

#[tokio::test]
async fn integration() {
    let path = unix_socket_path();
    let rpc_network = connect_unix_stream(path).await;
    let rpc_system = RpcSystem::new(Box::new(rpc_network), None);
    LocalSet::new()
        .run_until(async move {
            let (client, thread) = bootstrap(rpc_system).await;
            let mut echo = client.make_echo_request();
            echo.get().get_context().unwrap().set_thread(thread.clone());
            let echo_client_request = echo.send().promise.await.unwrap();
            let echo_client = echo_client_request.get().unwrap().get_result().unwrap();
            let mut echo_conf = echo_client.echo_request();
            echo_conf
                .get()
                .get_context()
                .unwrap()
                .set_thread(thread.clone());
            echo_conf.get().set_echo("Hello world");
            let echo_response = echo_conf.send().promise.await.unwrap();
            let text = echo_response
                .get()
                .unwrap()
                .get_result()
                .unwrap()
                .to_string()
                .unwrap();
            assert_eq!("Hello world", text);
        })
        .await;
}

/// Calling the deprecated makeMiningOld2 (@2) should return an error from the
/// server. Cap'n Proto requires sequential ordinals so this placeholder cannot
/// be removed, but the server intentionally rejects it.
#[tokio::test]
async fn make_mining_old2_rejected() {
    let path = unix_socket_path();
    let rpc_network = connect_unix_stream(path).await;
    let rpc_system = RpcSystem::new(Box::new(rpc_network), None);
    LocalSet::new()
        .run_until(async move {
            let (client, _thread) = bootstrap(rpc_system).await;
            let result = client.make_mining_old2_request().send().promise.await;
            assert!(
                result.is_err(),
                "makeMiningOld2 should be rejected by the server"
            );
        })
        .await;
}
