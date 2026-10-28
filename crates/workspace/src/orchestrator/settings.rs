//! Orchestrator settings › Profile (`Xir64`): which of the project's agent connections the Orchestrator runs on, its
//! model and reasoning effort, the choices each connection offers, and whether the chat can send with what is
//! published. They are versioned as agent profiles are: a change is saved as a draft, and *Publish vN* makes it the
//! version the chat answers on from its next reply.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  orchestrator::{OrchestratorSettings, runs_orchestrator},
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::runs_on::RunsOn;
use crate::{
  agents::form::{ModelOption, effort_options, model_options},
  messages::{self, Message},
};

/// A connection of the project the Orchestrator could run on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionChoice {
  pub id: ConnectionId,
  pub name: Arc<str>,
  pub provider: AgentProvider,
  /// Why the Orchestrator cannot run on it; `None` when it can.
  pub unavailable: Option<Message>,
}

/// Where the chat stands with what is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
  /// Nothing was chosen yet.
  NotChosen,
  /// The chosen connection was removed.
  ConnectionGone,
  /// The chosen connection cannot run the Orchestrator, as the message says.
  Unusable(Message),
  /// The chat sends on `connection` (its name) with `model`.
  Ready { connection: Arc<str>, model: ModelId },
}

impl Readiness {
  pub const fn is_ready(&self) -> bool {
    matches!(self, Self::Ready { .. })
  }
}

/// What is published, drafted and can be chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrchestratorSetup {
  /// The latest published version, which the chat answers on.
  pub settings: Option<OrchestratorSettings>,
  /// Its number.
  pub version: Option<u32>,
  /// The draft saved since.
  pub draft: Option<OrchestratorSettings>,
  connections: Vec<AgentConnection>,
}

impl OrchestratorSetup {
  /// Reads the project at `root`. Blocks on SQLite.
  pub fn load_in(root: &Path) -> Result<Self, Message> {
    let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
    let published = db
      .orchestrator_published()
      .map_err(|error| messages::store_error(&error))?;
    let draft = db.orchestrator_draft().map_err(|error| messages::store_error(&error))?;
    let connections = db
      .connections()
      .map_err(|error| messages::store_error(&error))?
      .into_iter()
      .filter_map(|connection| AgentConnection::try_from(connection).ok())
      .collect();
    let (version, settings) = published.unzip();
    Ok(Self {
      settings,
      version,
      draft,
      connections,
    })
  }

  /// The project's agent connections, by name, each with whether the Orchestrator can run on it.
  pub fn connections(&self) -> Vec<ConnectionChoice> {
    self
      .connections
      .iter()
      .map(|connection| ConnectionChoice {
        id: connection.id(),
        name: Arc::from(connection.name().as_str()),
        provider: connection.provider(),
        unavailable: unavailable(connection),
      })
      .collect()
  }

  /// The models `connection` offers, with `current` among them when its catalog does not list it.
  pub fn models(&self, connection: ConnectionId, current: Option<&ModelId>) -> Vec<ModelOption> {
    self
      .connection(connection)
      .map(|connection| model_options(connection, current))
      .unwrap_or_default()
  }

  /// The reasoning efforts `connection` lists for `model`.
  pub fn efforts(&self, connection: ConnectionId, model: &ModelId) -> Vec<String> {
    self
      .connection(connection)
      .map(|connection| effort_options(connection, model))
      .unwrap_or_default()
  }

  /// The reasoning efforts any model of `connection` lists, in their first order: what a model the connection does
  /// not list may take, unchecked.
  pub fn any_efforts(&self, connection: ConnectionId) -> Vec<String> {
    let Some(found) = self.connection(connection) else {
      return Vec::new();
    };
    let mut efforts: Vec<String> = Vec::new();
    for model in found.models() {
      for effort in effort_options(found, model.id()) {
        if !efforts.contains(&effort) {
          efforts.push(effort);
        }
      }
    }
    efforts
  }

  /// What the draft, or else the published version, holds: what the form starts from.
  pub fn current(&self) -> Option<&OrchestratorSettings> {
    self.draft.as_ref().or(self.settings.as_ref())
  }

  /// The first connection the Orchestrator can run on.
  pub fn first_usable(&self) -> Option<ConnectionId> {
    self
      .connections
      .iter()
      .find(|connection| unavailable(connection).is_none())
      .map(|connection| connection.id())
  }

  /// What choosing `connection` fills in: what the draft or published version holds on it, else its first listed
  /// model and the CLI's own effort. `None` when it offers no model to start with.
  pub fn choosing(&self, connection: ConnectionId) -> Option<OrchestratorSettings> {
    if let Some(settings) = self.current().filter(|settings| settings.connection() == connection) {
      return Some(settings.clone());
    }
    let model = self.connection(connection)?.models().first()?.id().clone();
    Some(OrchestratorSettings::new(connection, model, None))
  }

  /// Where the chat stands with what is chosen.
  pub fn readiness(&self) -> Readiness {
    let Some(settings) = &self.settings else {
      return Readiness::NotChosen;
    };
    let Some(connection) = self.connection(settings.connection()) else {
      return Readiness::ConnectionGone;
    };
    if let Some(reason) = unavailable(connection) {
      return Readiness::Unusable(reason);
    }
    Readiness::Ready {
      connection: Arc::from(connection.name().as_str()),
      model: settings.model().clone(),
    }
  }

  /// The provider of the published version's connection, which the chat's turns run on; `None` while there is none.
  pub fn provider(&self) -> Option<AgentProvider> {
    let settings = self.settings.as_ref()?;
    Some(self.connection(settings.connection())?.provider())
  }

  /// What `connection`'s CLI runs on; `None` for a connection the project does not have.
  pub fn runs_on(&self, connection: ConnectionId) -> Option<RunsOn> {
    let found = self.connection(connection)?;
    if let Some(provider) = found.api_provider() {
      return Some(RunsOn::ApiProvider {
        base_url: provider.base_url().clone(),
        key: provider.key(),
      });
    }
    Some(if found.api().is_some() {
      RunsOn::Native
    } else {
      RunsOn::SignIn(found.account().clone())
    })
  }

  fn connection(&self, id: ConnectionId) -> Option<&AgentConnection> {
    self.connections.iter().find(|connection| connection.id() == id)
  }
}

/// Whether settings are saved as the draft or published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
  Draft,
  Publish,
}

/// What to save, how, and where: the settings of the project at `root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saving {
  pub root: std::path::PathBuf,
  pub settings: OrchestratorSettings,
  pub keep: Keep,
}

/// Saves `settings` in the project at `root` as `keep` says, once its connection is one the Orchestrator can run
/// on: the published version's number when it was published. Blocks on SQLite.
pub fn save_in(root: &Path, settings: &OrchestratorSettings, keep: Keep) -> Result<Option<u32>, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let connection = db
    .connection(settings.connection())
    .map_err(|error| messages::store_error(&error))?
    .and_then(|connection| AgentConnection::try_from(connection).ok())
    .ok_or_else(|| Message::new("orchestrator.settings.connectionGone"))?;
  if let Some(reason) = unavailable(&connection) {
    return Err(reason);
  }
  let now = Timestamp::now();
  let saved = match keep {
    Keep::Draft => db.save_orchestrator_draft(settings, now).map(|()| None),
    Keep::Publish => db.publish_orchestrator(settings, now).map(Some),
  };
  saved.map_err(|error| messages::store_error(&error))
}

/// Why the Orchestrator cannot run on `connection`: not on a provider it does not run on. A connection on an API
/// provider runs it on the provider's key and host, as a step does (`docs/design/api-providers.md`).
fn unavailable(connection: &AgentConnection) -> Option<Message> {
  (!runs_orchestrator(connection.provider())).then(|| Message::new("orchestrator.settings.cliOnly"))
}
