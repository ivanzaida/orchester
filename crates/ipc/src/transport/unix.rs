//! Unix domain socket transport (macOS). The socket of a project's runner is
//! `$TMPDIR/orchester/runner-<hash>.sock`:
//!
//! - `$TMPDIR` is the user's own temporary folder on macOS (under `/var/folders`, mode 0700; launchd sets it for apps
//!   started from Finder too), and its paths stay well below the 104-byte limit of a socket path, which a folder in
//!   `~/Library` may not. Without `TMPDIR` it is `/tmp`, shared by all users; the mode check below then refuses a
//!   folder another user created first.
//! - The `orchester` folder in it is created with mode 0700 and set to 0700 again when it exists, which only its owner
//!   can do, so no other user can create, replace or connect to a socket in it.
//! - The runner removes a socket left behind by a runner that did not exit cleanly, but refuses one that some process
//!   still accepts connections on. It removes its own socket as soon as the desktop is connected: a runner accepts
//!   exactly one connection.
//! - The desktop checks that the process accepting its connection is the runner it started ([`server_process_id`], the
//!   socket's peer credentials), as on Windows.

use std::{
  fs::{self, DirBuilder, Permissions},
  io,
  os::unix::fs::{DirBuilderExt, PermissionsExt},
  path::{Path, PathBuf},
};

use tokio::net::{UnixListener, UnixStream};

use super::fnv1a;
use crate::IpcError;

/// The runner's end of a connection.
pub type ServerStream = UnixStream;
/// The desktop's end of a connection.
pub type ClientStream = UnixStream;

/// The folder of the runner sockets in the user's temporary folder.
const SOCKET_FOLDER: &str = "orchester";
/// Only the owner may list, create or connect to anything in the socket folder.
const PRIVATE: u32 = 0o700;

/// Where a project's runner listens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
  path: PathBuf,
  name: String,
}

impl Endpoint {
  pub fn for_project(project_root: &Path) -> Result<Self, IpcError> {
    let canonical = project_root.canonicalize().map_err(|source| IpcError::ProjectRoot {
      path: project_root.to_path_buf(),
      source,
    })?;
    let hash = fnv1a(canonical.as_os_str().as_encoded_bytes());
    let path = socket_folder().join(format!("runner-{hash:016x}.sock"));
    Ok(Self {
      name: path.display().to_string(),
      path,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  /// The socket file.
  pub fn path(&self) -> &Path {
    &self.path
  }

  /// Binds the socket and waits for the desktop to connect. Fails if another process accepts
  /// connections on this socket, so a stray process cannot squat on it. The socket file is
  /// removed once the desktop is connected, or when accepting is given up.
  pub async fn accept(&self) -> Result<ServerStream, IpcError> {
    let folder = self
      .path
      .parent()
      .expect("the socket path is a file in the socket folder");
    private_folder(folder).map_err(|source| IpcError::EndpointFolder {
      path: folder.to_path_buf(),
      source,
    })?;
    self.remove_stale().await?;
    let listener = UnixListener::bind(&self.path).map_err(|source| {
      if source.kind() == io::ErrorKind::AddrInUse {
        self.in_use()
      } else {
        self.error(source)
      }
    })?;
    let _socket = SocketFile(&self.path);
    let (stream, _) = listener.accept().await.map_err(|source| self.error(source))?;
    Ok(stream)
  }

  /// One connection attempt; see [`Endpoint::not_ready`] for the failures worth retrying.
  pub async fn connect(&self) -> io::Result<ClientStream> {
    UnixStream::connect(&self.path).await
  }

  /// Whether a failed [`Endpoint::connect`] is worth retrying: the runner has not bound its
  /// socket yet (`NotFound`), or the file is a stale socket it has not replaced yet.
  pub fn not_ready(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused)
  }

  /// Removes a socket file nobody accepts connections on: a runner before this one exited
  /// without removing it. The project's runner lock is held, so no runner of this project uses it.
  async fn remove_stale(&self) -> Result<(), IpcError> {
    match UnixStream::connect(&self.path).await {
      Ok(_) => Err(self.in_use()),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
      Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
        tracing::info!("removing the stale runner socket {}", self.name);
        fs::remove_file(&self.path).map_err(|source| self.error(source))
      }
      Err(error) => Err(self.error(error)),
    }
  }

  fn in_use(&self) -> IpcError {
    IpcError::EndpointInUse {
      name: self.name.clone(),
    }
  }

  fn error(&self, source: io::Error) -> IpcError {
    IpcError::Endpoint {
      name: self.name.clone(),
      source,
    }
  }
}

/// The folder holding the runner sockets of this user.
fn socket_folder() -> PathBuf {
  std::env::temp_dir().join(SOCKET_FOLDER)
}

/// Creates `folder` with mode 0700, or makes an existing one 0700. Changing the mode succeeds
/// only for the folder's owner, so an existing folder of another user is refused.
fn private_folder(folder: &Path) -> io::Result<()> {
  match DirBuilder::new().mode(PRIVATE).create(folder) {
    Ok(()) => {}
    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
    Err(error) => return Err(error),
  }
  if !fs::symlink_metadata(folder)?.is_dir() {
    return Err(io::Error::new(
      io::ErrorKind::AlreadyExists,
      "the socket folder exists and is not a folder",
    ));
  }
  fs::set_permissions(folder, Permissions::from_mode(PRIVATE))
}

/// Removes the bound socket file when accepting ends, whether a desktop connected or not.
struct SocketFile<'a>(&'a Path);

impl Drop for SocketFile<'_> {
  fn drop(&mut self) {
    if let Err(error) = fs::remove_file(self.0) {
      tracing::warn!("cannot remove the runner socket {}: {error}", self.0.display());
    }
  }
}

/// The process accepting the desktop's connection, from the socket's peer credentials
/// (`LOCAL_PEEREPID` on macOS).
pub fn server_process_id(stream: &ClientStream) -> io::Result<u32> {
  let pid = stream
    .peer_cred()?
    .pid()
    .ok_or_else(|| io::Error::other("the system did not report the process at the other end of the socket"))?;
  u32::try_from(pid).map_err(|_| io::Error::other(format!("invalid peer process id {pid}")))
}
