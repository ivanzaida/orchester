//! The app's MCP server as startup and Settings › Automation run it: started once with the window, and in the
//! installed app turned on and off, moved to another port and given a new token in place, without restarting the
//! app. Every change is published in [`McpStatus`], which the status bar and Settings show. The actions that change
//! it block (the database, the keychain, the port): the window calls them off its thread.

use std::{num::NonZeroU16, path::PathBuf, sync::Arc};

use lurq::{app::Tree, mcp::McpConfig};
use orchester_core::{app_mcp::McpPort, secret::VaultError};
use orchester_store::StoreError;
use orchester_workspace::{
  messages::{self, Message},
  secrets::Vault,
  workspace::Workspace,
};
use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch};

use super::{
  clients::Clients,
  discovery::{DiscoveryFile, DiscoveryFolder},
  input_gate::InputGate,
  mode::McpMode,
  server::{BindError, Listening, Serving, endpoint},
  settings::McpSettings,
  status::{McpKind, McpStatus, ServerState},
  token::{BearerToken, TokenStore},
};

/// Why a change to the installed app's server was not made.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The secret store refused the new token; the old one stays in use.
  #[error(transparent)]
  Vault(#[from] VaultError),
  /// Only the installed app's server has a fixed address and a stored token.
  #[error("this copy's MCP server takes a new port and token on each start")]
  NotInstallation,
}

impl ControlError {
  /// What the window says about it.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::Vault(error) => messages::vault_error(error),
      Self::NotInstallation => messages::app_mcp_not_installation(),
    }
  }
}

/// The app's MCP server; clones control the same one. Stopped by [`AppMcp::shutdown`], or when the last clone goes.
#[derive(Clone)]
pub struct AppMcp(Arc<Inner>);

impl PartialEq for AppMcp {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for AppMcp {}

struct Inner {
  runtime: Handle,
  serving: Serving,
  source: Source,
  /// Held while the server changes, so two changes never interleave.
  running: Mutex<Running>,
  /// The token in use; apart from `running`, so reading it never waits on the keychain.
  token: Mutex<Option<BearerToken>>,
  /// Held while the stored token is read, made or replaced, so a read that started before a rotation cannot put the
  /// old token back in `token`, and two first uses cannot make two tokens. Taken after `running`, never before.
  creation: Mutex<()>,
  status: watch::Sender<McpStatus>,
}

enum Source {
  Installation { workspace: Workspace, vault: Vault },
  Discovery(DiscoveryFolder),
}

#[derive(Default)]
struct Running {
  listening: Option<Listening>,
  discovery: Option<DiscoveryFile>,
}

impl AppMcp {
  /// Hosts the tools of `config` on `tree`, the window's root tree, and serves them on `runtime` as `mode` says.
  /// What came of it is in [`status`](Self::status): a server that cannot listen leaves the window without MCP,
  /// and the log says why.
  pub fn start(
    tree: &mut Tree,
    runtime: &Handle,
    mode: McpMode,
    (config, settings): (McpConfig, McpSettings),
    gate: &InputGate,
  ) -> Self {
    let tools = tree.enable_mcp(config.hosted());
    gate.attach(tools.clone());
    let (kind, source) = match mode {
      McpMode::Installation { workspace, vault } => (McpKind::Installation, Source::Installation { workspace, vault }),
      McpMode::Qa(folder) => (
        McpKind::Qa { folder: folder.clone() },
        Source::Discovery(DiscoveryFolder::At(folder)),
      ),
      McpMode::Development => (
        McpKind::Development { file: None },
        Source::Discovery(DiscoveryFolder::User),
      ),
    };
    let (status, _) = watch::channel(McpStatus::unavailable(kind, settings));
    let serving = Serving {
      gate: gate.clone(),
      tools,
      clients: Clients::new(status.clone()),
    };
    let server = Self(Arc::new(Inner {
      runtime: runtime.clone(),
      serving,
      source,
      running: Mutex::new(Running::default()),
      token: Mutex::new(None),
      creation: Mutex::new(()),
      status,
    }));
    let mut running = server.0.running.lock();
    server.0.up(&mut running);
    drop(running);
    server
  }

  pub fn status(&self) -> McpStatus {
    self.0.status.borrow().clone()
  }

  /// The status now and each change after.
  pub fn subscribe(&self) -> watch::Receiver<McpStatus> {
    self.0.status.subscribe()
  }

  /// The port the server listens on, while it listens.
  pub fn port(&self) -> Option<NonZeroU16> {
    self.status().listening_port()
  }

  /// This process's discovery file, for a QA copy, a test or a development build while it listens.
  pub fn discovery_path(&self) -> Option<PathBuf> {
    self
      .0
      .running
      .lock()
      .discovery
      .as_ref()
      .map(|file| file.path().to_owned())
  }

  /// The token in use, for Show and Copy on a person's click in the window; `None` before one could be read.
  pub fn token(&self) -> Option<BearerToken> {
    self.0.token.lock().clone()
  }

  /// The token in use, read from the secret store (or a first one stored there) when the server has not read it
  /// yet, e.g. while it is off; for Show and Copy on a person's click. Blocks.
  pub fn read_token(&self) -> Result<BearerToken, ControlError> {
    self.0.token_or_read()
  }

  /// Turns the installed app's server on or off and keeps the choice. Blocks.
  pub fn set_enabled(&self, enabled: bool) -> Result<(), ControlError> {
    let workspace = self.0.installation()?;
    let mut running = self.0.running.lock();
    workspace.set_mcp_server_enabled(enabled)?;
    self.0.down(&mut running);
    if enabled {
      self.0.up(&mut running);
    } else {
      self.0.publish(ServerState::Off);
    }
    Ok(())
  }

  /// Keeps `port` as the installed app's port and listens there from now on; a port another program holds is
  /// kept too, and the server says it is taken rather than moving on. Blocks.
  pub fn use_port(&self, port: McpPort) -> Result<(), ControlError> {
    let workspace = self.0.installation()?;
    let mut running = self.0.running.lock();
    workspace.set_mcp_port(port)?;
    self.0.down(&mut running);
    self.0.up(&mut running);
    Ok(())
  }

  /// Stores a new token in place of the old one and serves with it from now on; the sessions of the old one end
  /// and the list of clients starts again. A token the secret store refuses changes nothing. Blocks.
  pub fn rotate_token(&self) -> Result<(), ControlError> {
    let tokens = self.0.token_store()?;
    let mut running = self.0.running.lock();
    let token = {
      let _creating = self.0.creation.lock();
      let token = tokens.replace()?;
      *self.0.token.lock() = Some(token.clone());
      token
    };
    tracing::info!("the app's MCP token was replaced");
    self.0.serving.clients.start_again();
    self.0.status.send_modify(|status| status.rotated = true);
    match running.listening.as_ref() {
      Some(listening) => listening.rotate(token),
      // A server that could not read its token starts with the new one.
      None => self.0.up(&mut running),
    }
    Ok(())
  }

  /// Starts the server again as its settings say, reading the token again if it could not be read. Blocks.
  pub fn try_again(&self) {
    let mut running = self.0.running.lock();
    if running.listening.is_none() {
      let _creating = self.0.creation.lock();
      *self.0.token.lock() = None;
    }
    self.0.down(&mut running);
    self.0.up(&mut running);
  }

  /// Stops the server and removes its discovery file. Blocks.
  pub fn shutdown(&self) {
    let mut running = self.0.running.lock();
    self.0.down(&mut running);
  }
}

impl Inner {
  fn installation(&self) -> Result<&Workspace, ControlError> {
    match &self.source {
      Source::Installation { workspace, .. } => Ok(workspace),
      Source::Discovery(_) => Err(ControlError::NotInstallation),
    }
  }

  fn token_store(&self) -> Result<TokenStore, ControlError> {
    match &self.source {
      Source::Installation { workspace, vault } => Ok(TokenStore::new(vault.clone(), workspace.installation_id()?)),
      Source::Discovery(_) => Err(ControlError::NotInstallation),
    }
  }

  /// Starts the server as the source says and publishes what came of it.
  fn up(&self, running: &mut Running) {
    let state = match &self.source {
      Source::Installation { workspace, .. } => self.up_installation(running, workspace),
      Source::Discovery(folder) => self.up_discovery(running, folder),
    };
    if let ServerState::Listening { port } = state {
      tracing::info!(endpoint = %endpoint(port), "the app's MCP server is listening");
    }
    self.publish(state);
  }

  fn up_installation(&self, running: &mut Running, workspace: &Workspace) -> ServerState {
    let settings = workspace
      .mcp_server_enabled()
      .and_then(|enabled| Ok((enabled, workspace.mcp_port()?)));
    let port = match settings {
      Ok((enabled, port)) => {
        self
          .status
          .send_if_modified(|status| status.port.replace(port) != Some(port));
        if !enabled {
          return ServerState::Off;
        }
        port
      }
      Err(error) => {
        tracing::error!(%error, "the app's MCP server settings cannot be read");
        return ServerState::Unavailable;
      }
    };
    let token = match self.token_or_read() {
      Ok(token) => token,
      Err(ControlError::Vault(error)) => {
        tracing::error!(%error, "the app's MCP token cannot be read from the secret store");
        return ServerState::TokenUnreadable {
          reason: messages::vault_error(&error),
        };
      }
      Err(error) => {
        tracing::error!(%error, "the app's MCP token cannot be read");
        return ServerState::Unavailable;
      }
    };
    let fixed = NonZeroU16::new(port.get()).expect("an MCP port is 1024 or more");
    match Listening::start(&self.runtime, Some(fixed), self.serving.clone(), token) {
      Ok(listening) => {
        running.listening = Some(listening);
        ServerState::Listening { port: fixed }
      }
      Err(BindError::Taken) => {
        tracing::warn!(
          port = port.get(),
          "the app's MCP port is taken; the server does not listen"
        );
        ServerState::PortTaken { port }
      }
      Err(BindError::Other(error)) => {
        tracing::error!(%error, port = port.get(), "the app's MCP server did not start");
        ServerState::Unavailable
      }
    }
  }

  /// A free port and a new token, written to this process's discovery file in `folder`.
  fn up_discovery(&self, running: &mut Running, folder: &DiscoveryFolder) -> ServerState {
    let Some(folder) = folder.path() else {
      return ServerState::Unavailable;
    };
    let token = BearerToken::generate();
    *self.token.lock() = Some(token.clone());
    let listening = match Listening::start(&self.runtime, None, self.serving.clone(), token.clone()) {
      Ok(listening) => listening,
      Err(error) => {
        tracing::error!(%error, "the app's MCP server did not start");
        return ServerState::Unavailable;
      }
    };
    let port = listening.port();
    let discovery = match DiscoveryFile::write(&folder, port, token.expose()) {
      Ok(discovery) => discovery,
      Err(error) => {
        tracing::error!(%error, folder = %folder.display(), "the app's MCP server wrote no discovery file");
        listening.stop();
        return ServerState::Unavailable;
      }
    };
    tracing::info!(discovery = %discovery.path().display(), "the app's MCP discovery file");
    let file = discovery.path().to_owned();
    self.status.send_modify(|status| {
      if let McpKind::Development { file: shown } = &mut status.kind {
        *shown = Some(file);
      }
    });
    running.listening = Some(listening);
    running.discovery = Some(discovery);
    ServerState::Listening { port }
  }

  /// The token in use; read from the secret store, or a first one stored there, when there is none yet.
  fn token_or_read(&self) -> Result<BearerToken, ControlError> {
    let _creating = self.creation.lock();
    if let Some(token) = self.token.lock().clone() {
      return Ok(token);
    }
    let token = self.token_store()?.read_or_create()?;
    // Still none: a rotation takes the same lock, so nothing filled it meanwhile.
    *self.token.lock() = Some(token.clone());
    Ok(token)
  }

  fn down(&self, running: &mut Running) {
    if let Some(listening) = running.listening.take() {
      listening.stop();
    }
    running.discovery = None;
  }

  fn publish(&self, state: ServerState) {
    self.status.send_if_modified(|status| {
      let changed = status.server != state;
      status.server = state;
      changed
    });
  }
}
