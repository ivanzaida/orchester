//! The window with a keychain in memory and links recorded, a team server and a stand-in cloud on loopback, and a
//! recording port in front of the server that lists every request line the app sent it.

use std::{
  collections::HashMap,
  io::{Read, Write},
  net::{Shutdown, SocketAddr, TcpListener, TcpStream},
  path::Path,
  process::Command,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
};

use orchester_app::{routes::Route, startup::Launch};
use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use orchester_workspace::secrets::Vault;
use parking_lot::Mutex;

use super::support::{Data, Window};

/// The window is tall enough that every panel's parts are laid out without scrolling.
const HEIGHT: u32 = 2400;

/// A keychain in memory: no test touches the keychain of the user running it.
#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// The window of a new data folder with Kontur in a git repository whose `origin` is `origin`, a keychain in memory
/// and links recorded instead of opened.
pub fn window(origin: Option<&str>) -> Window {
  let data = Data::new();
  let vault = Arc::new(MemoryVault::default());
  let runner = data.dir.path().join("no-runner").join("orchester-runner");
  let launch = Launch::begin_with_vault(
    &data.database(),
    runner,
    data.runtime.handle(),
    (None, Vault::with(vault)),
  );
  let repo = data.repo("repo-kontur");
  git(&repo, &["init", "-q"]);
  if let Some(origin) = origin {
    git(&repo, &["remote", "add", "origin", origin]);
  }
  data.add_kontur(launch.workspace().expect("the session started"));
  launch.services().expect("services").links.record_instead();
  let mut window = Window::of(data, launch);
  window.tree.resize(1440, HEIGHT);
  window.pass();
  window
}

fn git(repo: &Path, args: &[&str]) {
  let status = Command::new("git")
    .args(args)
    .current_dir(repo)
    .status()
    .expect("git runs");
  assert!(status.success(), "git {args:?}");
}

/// Goes to `route` and lays the window out until `id` shows.
pub fn go(window: &mut Window, route: Route, id: &str) {
  window.navigation().go(route);
  shows(window, id);
}

/// Lays the window out until `id` shows.
pub fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

/// Presses `id` once it shows.
pub fn press(window: &mut Window, id: &str) {
  shows(window, id);
  window.press(id);
}

/// The services' runtime runs `work`.
pub fn block_on<T>(window: &Window, work: impl Future<Output = T>) -> T {
  window.data.runtime.block_on(work)
}

/// A loopback port in front of `target` that records the request line of every HTTP request passing through.
pub struct Recorder {
  pub port: u16,
  shared: Arc<Shared>,
}

/// What the port's relays share.
#[derive(Default)]
struct Shared {
  lines: Mutex<Vec<String>>,
  /// The server looks down: new connections are closed, and open ones end at their next request.
  down: AtomicBool,
  /// The next request whose line starts with this ends without an answer, as if the server went away.
  refuse: Mutex<Option<String>>,
}

impl Recorder {
  pub fn start(target: SocketAddr) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let port = listener.local_addr().expect("address").port();
    let shared = Arc::new(Shared::default());
    let relaying = shared.clone();
    thread::spawn(move || {
      for client in listener.incoming().flatten() {
        if relaying.down.load(Ordering::SeqCst) {
          let _ = client.shutdown(Shutdown::Both);
          continue;
        }
        let Ok(server) = TcpStream::connect(target) else {
          // The connection ends as if the server were down; the test sees it fail where it expects an answer.
          let _ = client.shutdown(Shutdown::Both);
          continue;
        };
        relay(client, server, &relaying);
      }
    });
    Self { port, shared }
  }

  /// From now on the server can't be reached through the port.
  pub fn go_down(&self) {
    self.shared.down.store(true, Ordering::SeqCst);
  }

  /// The next request whose line starts with `line` (`POST /api/v1/auth/cloud`) ends without an answer.
  pub fn refuse_once(&self, line: &str) {
    *self.shared.refuse.lock() = Some(line.to_owned());
  }

  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.port)
  }

  /// The request lines so far, e.g. `POST /api/v1/auth/code`.
  pub fn lines(&self) -> Vec<String> {
    self.shared.lines.lock().clone()
  }
}

/// Copies both ways, recording the request lines the client sends.
fn relay(client: TcpStream, server: TcpStream, shared: &Arc<Shared>) {
  let (mut client_read, mut server_write) = (client.try_clone().expect("client"), server.try_clone().expect("server"));
  let shared = shared.clone();
  thread::spawn(move || {
    let mut buffer = [0_u8; 16 * 1024];
    while let Ok(read) = client_read.read(&mut buffer) {
      if shared.down.load(Ordering::SeqCst) || refused(&shared, &buffer[..read]) {
        let _ = client_read.shutdown(Shutdown::Both);
        break;
      }
      if read == 0 || server_write.write_all(&buffer[..read]).is_err() {
        break;
      }
      let text = String::from_utf8_lossy(&buffer[..read]);
      for line in text.lines() {
        let method = line.split(' ').next().unwrap_or_default();
        if ["GET", "POST", "PATCH", "PUT", "DELETE"].contains(&method) && line.ends_with("HTTP/1.1") {
          shared.lines.lock().push(line.trim_end_matches(" HTTP/1.1").to_owned());
        }
      }
    }
    // The server sees the client go.
    let _ = server_write.shutdown(Shutdown::Write);
  });
  let (mut server_read, mut client_write) = (server, client);
  thread::spawn(move || {
    let mut buffer = [0_u8; 16 * 1024];
    while let Ok(read) = server_read.read(&mut buffer) {
      if read == 0 || client_write.write_all(&buffer[..read]).is_err() {
        break;
      }
    }
    // The client sees the server go.
    let _ = client_write.shutdown(Shutdown::Write);
  });
}

/// Whether `chunk` starts the request the port refuses once; the refusal is used up then.
fn refused(shared: &Shared, chunk: &[u8]) -> bool {
  let mut refuse = shared.refuse.lock();
  let hit = refuse
    .as_deref()
    .is_some_and(|line| String::from_utf8_lossy(chunk).starts_with(line));
  if hit {
    *refuse = None;
  }
  hit
}

/// Whether the request line `line` names an endpoint only the server admin calls: people, devices of others,
/// invites, the server's settings and cloud link, and projects and their members made or changed.
pub fn administrative(line: &str) -> bool {
  let (method, path) = line.split_once(' ').unwrap_or((line, ""));
  let path = path.split('?').next().unwrap_or_default();
  path.starts_with("/api/v1/admin/")
    || path.starts_with("/api/v1/invites")
    || path.starts_with("/api/v1/server/")
    || (method == "POST" && path == "/api/v1/projects")
    || (method == "PATCH" && path.starts_with("/api/v1/projects/") && path.matches('/').count() == 4)
    || (matches!(method, "POST" | "PATCH") && path.contains("/members"))
}
