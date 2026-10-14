//! Whether a project can run its first task, as the new Overview's empty project lists it (`Ready`
//! `VY4QP` in `a5CcQx`): a published agent, a published pipeline and an agent connection that is signed
//! in. Each check says what it found, or why it is not ready.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::ConnectionStatus,
  resource::{AgentProfile, Pipeline, ResourceKind, VersionNumber},
  store::{ConnectionStore, ResourceStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::{connections::data::agent_connections, messages::Message};

/// What a project has to run a task with, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectReadiness {
  /// The roles of the published agent profiles, in the store's order.
  pub agents: Vec<Arc<str>>,
  /// The published pipelines with their latest version.
  pub pipelines: Vec<(Arc<str>, VersionNumber)>,
  /// The agent connections with how each stands.
  pub connections: Vec<(Arc<str>, ConnectionStatus)>,
}

/// One line of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
  Agents,
  Pipeline,
  Connection,
}

impl CheckKind {
  pub const ALL: [Self; 3] = [Self::Agents, Self::Pipeline, Self::Connection];

  /// The line's name (*Agents*).
  pub fn label(self) -> Message {
    Message::new(match self {
      Self::Agents => "overview.ready.agents.label",
      Self::Pipeline => "overview.ready.pipeline.label",
      Self::Connection => "overview.ready.connection.label",
    })
  }
}

/// A check and what it found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadyCheck {
  pub kind: CheckKind,
  pub ready: bool,
  /// What is there (*Explorer, Planner*), or what is missing.
  pub detail: Message,
}

impl ProjectReadiness {
  pub fn load<S>(store: &S) -> Result<Self, StoreError>
  where
    S: ResourceStore<Error = StoreError> + ConnectionStore<Error = StoreError> + SecretStore<Error = StoreError>,
  {
    let mut agents = Vec::new();
    for resource in store.resources(ResourceKind::AgentProfile)? {
      if let Some(version) = store.latest_version::<AgentProfile>(resource.id())? {
        agents.push(version.body().role().as_str().into());
      }
    }
    let mut pipelines = Vec::new();
    for resource in store.resources(ResourceKind::Pipeline)? {
      if let Some(version) = store.latest_version::<Pipeline>(resource.id())? {
        pipelines.push((resource.name().as_str().into(), version.number()));
      }
    }
    let secrets = store.secrets()?;
    let connections = agent_connections(store.connections()?)
      .iter()
      .map(|connection| (connection.name().as_str().into(), connection.status_with(&secrets)))
      .collect();
    Ok(Self {
      agents,
      pipelines,
      connections,
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  /// The three checks in [`CheckKind::ALL`]'s order; names in a line joined by `separator`.
  pub fn checks(&self, separator: &str) -> [ReadyCheck; 3] {
    CheckKind::ALL.map(|kind| self.check(kind, separator))
  }

  fn check(&self, kind: CheckKind, separator: &str) -> ReadyCheck {
    let (ready, detail) = match kind {
      CheckKind::Agents if self.agents.is_empty() => (false, Message::new("overview.ready.agents.none")),
      CheckKind::Agents => (
        true,
        Message::new("overview.ready.agents.published").with("agents", self.agents.join(separator)),
      ),
      CheckKind::Pipeline => match self.pipelines.as_slice() {
        [] => (false, Message::new("overview.ready.pipeline.none")),
        [(name, version)] => (
          true,
          Message::new("overview.ready.pipeline.one")
            .with("pipeline", name.to_string())
            .with("version", version.to_string()),
        ),
        several => (
          true,
          Message::new("overview.ready.pipeline.several").with("count", several.len().to_string()),
        ),
      },
      CheckKind::Connection => self.connection(separator),
    };
    ReadyCheck { kind, ready, detail }
  }

  /// Ready when a connection is signed in; otherwise why the first one is not.
  fn connection(&self, separator: &str) -> (bool, Message) {
    let signed_in: Vec<&str> = self
      .connections
      .iter()
      .filter(|(_, status)| *status == ConnectionStatus::Connected)
      .map(|(name, _)| &**name)
      .collect();
    if !signed_in.is_empty() {
      let names = signed_in.join(separator);
      return (
        true,
        Message::new("overview.ready.connection.signedIn").with("connections", names),
      );
    }
    match self.connections.first() {
      None => (false, Message::new("overview.ready.connection.none")),
      Some((name, status)) => (
        false,
        Message::new(not_ready_key(*status)).with("connection", name.to_string()),
      ),
    }
  }
}

/// Why a connection that is not signed in cannot run agents yet.
const fn not_ready_key(status: ConnectionStatus) -> &'static str {
  match status {
    ConnectionStatus::NotChecked | ConnectionStatus::Connected => "overview.ready.connection.notChecked",
    ConnectionStatus::SignInRequired => "overview.ready.connection.signInRequired",
    ConnectionStatus::SignInUnknown => "overview.ready.connection.signInUnknown",
    ConnectionStatus::ExecutableMissing => "overview.ready.connection.executableMissing",
    ConnectionStatus::NotRecognized => "overview.ready.connection.notRecognized",
    ConnectionStatus::CouldNotRun => "overview.ready.connection.couldNotRun",
    ConnectionStatus::KeyMissing => "overview.ready.connection.keyMissing",
    ConnectionStatus::KeyRejected => "overview.ready.connection.keyRejected",
    ConnectionStatus::BillingProblem => "overview.ready.connection.billingProblem",
    ConnectionStatus::EndpointUnreachable => "overview.ready.connection.endpointUnreachable",
  }
}
