//! The Unix domain socket endpoint: its private folder, stale and squatted sockets, and the
//! peer check the desktop makes.

use std::{
  fs::{self, DirBuilder, Permissions},
  os::unix::{
    fs::{DirBuilderExt, PermissionsExt},
    net::UnixListener,
  },
  time::Duration,
};

use orchester_ipc::{ClientStream, Endpoint, IpcError, ServerStream, server_process_id};
use tempfile::TempDir;
use tokio::{task::JoinHandle, time::timeout};

const PATIENCE: Duration = Duration::from_secs(10);

fn endpoint() -> (TempDir, Endpoint) {
  let project = tempfile::tempdir().expect("temporary directory");
  let endpoint = Endpoint::for_project(project.path()).expect("endpoint");
  (project, endpoint)
}

fn accepting(endpoint: &Endpoint) -> JoinHandle<Result<ServerStream, IpcError>> {
  let endpoint = endpoint.clone();
  tokio::spawn(async move { endpoint.accept().await })
}

/// Connects once the endpoint accepts, as the desktop does.
async fn connect(endpoint: &Endpoint) -> ClientStream {
  timeout(PATIENCE, async {
    loop {
      match endpoint.connect().await {
        Ok(stream) => return stream,
        Err(error) if Endpoint::not_ready(&error) => tokio::time::sleep(Duration::from_millis(10)).await,
        Err(error) => panic!("cannot connect: {error}"),
      }
    }
  })
  .await
  .expect("connected in time")
}

fn socket_folder(endpoint: &Endpoint) -> &std::path::Path {
  endpoint.path().parent().expect("the socket is in a folder")
}

fn mode(path: &std::path::Path) -> u32 {
  fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

#[tokio::test]
async fn socket_accepts_one_connection_from_this_user_and_is_removed_then() {
  let (_project, endpoint) = endpoint();
  let server = accepting(&endpoint);

  let client = connect(&endpoint).await;
  let _server = timeout(PATIENCE, server)
    .await
    .expect("accepted in time")
    .expect("accept task")
    .expect("accept");

  assert_eq!(
    server_process_id(&client).expect("peer process"),
    std::process::id(),
    "the peer check names the process that accepted"
  );
  assert!(!endpoint.path().exists(), "the socket is removed once connected");
  let second = endpoint
    .connect()
    .await
    .expect_err("nobody accepts a second connection");
  assert!(Endpoint::not_ready(&second), "{second}");
}

#[tokio::test]
async fn socket_folder_open_to_others_is_made_private_again() {
  let (_project, endpoint) = endpoint();
  let folder = socket_folder(&endpoint);
  DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(folder)
    .expect("socket folder");
  fs::set_permissions(folder, Permissions::from_mode(0o755)).expect("open the folder up");

  let server = accepting(&endpoint);
  let _client = connect(&endpoint).await;
  timeout(PATIENCE, server)
    .await
    .expect("accepted in time")
    .expect("accept task")
    .expect("accept");

  assert_eq!(mode(folder), 0o700);
}

#[tokio::test]
async fn stale_socket_of_an_earlier_runner_is_replaced() {
  let (_project, endpoint) = endpoint();
  DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(socket_folder(&endpoint))
    .expect("socket folder");
  // A runner that was killed leaves its socket file behind; nobody accepts on it.
  drop(UnixListener::bind(endpoint.path()).expect("bind the earlier socket"));
  assert!(endpoint.path().exists());

  let server = accepting(&endpoint);
  let _client = connect(&endpoint).await;
  timeout(PATIENCE, server)
    .await
    .expect("accepted in time")
    .expect("accept task")
    .expect("the stale socket is replaced");
}

#[tokio::test]
async fn socket_another_process_accepts_on_is_refused() {
  let (_project, endpoint) = endpoint();
  DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(socket_folder(&endpoint))
    .expect("socket folder");
  let squatter = UnixListener::bind(endpoint.path()).expect("squat on the socket");

  let result = timeout(PATIENCE, endpoint.accept())
    .await
    .expect("accept returns in time");
  assert!(
    matches!(result, Err(IpcError::EndpointInUse { .. })),
    "{:?}",
    result.err()
  );
  drop(squatter);
  fs::remove_file(endpoint.path()).expect("remove the squatter's socket");
}
