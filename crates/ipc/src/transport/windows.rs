//! Windows named-pipe transport. The pipe name is derived from the canonical project root, so
//! the desktop and the runner of one project agree on it without passing it around, and
//! different projects never share one.

use std::{io, path::Path};

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};

use crate::IpcError;

/// The runner's end of a connection.
pub type ServerStream = NamedPipeServer;
/// The desktop's end of a connection.
pub type ClientStream = NamedPipeClient;

/// Where a project's runner listens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
  name: String,
}

impl Endpoint {
  pub fn for_project(project_root: &Path) -> Result<Self, IpcError> {
    let canonical = project_root.canonicalize().map_err(|source| IpcError::ProjectRoot {
      path: project_root.to_path_buf(),
      source,
    })?;
    // Windows paths are case-insensitive; fold ASCII case so two spellings of one path agree.
    let key = canonical.as_os_str().as_encoded_bytes().to_ascii_lowercase();
    Ok(Self {
      name: format!(r"\\.\pipe\orchester-runner-{:016x}", fnv1a(&key)),
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  /// Creates the only instance of the pipe and waits for the desktop to connect. Fails if any
  /// other process already owns a pipe of this name, so a stray process cannot squat on it.
  pub async fn accept(&self) -> Result<ServerStream, IpcError> {
    let server = ServerOptions::new()
      .first_pipe_instance(true)
      .reject_remote_clients(true)
      .create(&self.name)
      .map_err(|source| {
        if source.kind() == io::ErrorKind::PermissionDenied {
          IpcError::EndpointInUse {
            name: self.name.clone(),
          }
        } else {
          self.error(source)
        }
      })?;
    server.connect().await.map_err(|source| self.error(source))?;
    Ok(server)
  }

  /// One connection attempt. `NotFound` means the runner has not created the pipe yet.
  pub fn connect(&self) -> io::Result<ClientStream> {
    ClientOptions::new().open(&self.name)
  }

  fn error(&self, source: io::Error) -> IpcError {
    IpcError::Endpoint {
      name: self.name.clone(),
      source,
    }
  }
}

/// 64-bit FNV-1a. Stable across builds, unlike `std`'s `DefaultHasher`, which matters because
/// both processes must compute the same name.
fn fnv1a(bytes: &[u8]) -> u64 {
  const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const PRIME: u64 = 0x0000_0100_0000_01b3;
  bytes
    .iter()
    .fold(OFFSET, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(PRIME))
}
