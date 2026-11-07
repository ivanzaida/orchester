//! The answer of a start that does not start because its pipeline is not ready on this computer (`not_ready_here`,
//! `docs/design/team-configuration.md`, "What a computer fills in"): the parts that are missing or cannot be known,
//! each with a message that says the fix. Shared by every start: a team start, and a local one that refuses the same
//! way. A part holds names and ids only: no secret value, and no path but the program path the person set themselves
//! for a team MCP server whose file is gone, which they need to see to fix it.

use orchester_core::{
  id::ResourceId,
  sharing::{ConnectionProblem, LocalProgramProblem, Missing, ProgramProblem},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{PipelineReadiness, ProjectReadiness, Undecided};
use crate::messages::Message;

/// One part a pipeline lacks, with the name of the profile or MCP server it is about (the secret's own name for a
/// secret). Kept with a start that failed on it, so it is serializable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingPart {
  pub missing: Missing,
  pub name: String,
}

impl MissingPart {
  /// `missing`, its resource named by `name_of`.
  pub fn new(missing: Missing, name_of: &dyn Fn(ResourceId) -> String) -> Self {
    let name = match &missing {
      Missing::Connection { profile, .. } => name_of(*profile),
      Missing::McpOff { server } | Missing::McpProgram { server, .. } => name_of(*server),
      Missing::Secret { name } | Missing::LocalSecret { name, .. } => name.as_str().to_owned(),
      Missing::LocalMcpProgram { name, .. } => name.as_str().to_owned(),
    };
    Self { missing, name }
  }

  /// What is missing and what to do about it.
  pub fn message(&self) -> Message {
    let named = |key: &'static str, argument: &'static str| Message::new(key).with(argument, self.name.clone());
    match &self.missing {
      Missing::Connection { cli, problem, .. } => {
        let key = match problem {
          ConnectionProblem::NoneOfKind => "notReadyHere.connectionNoneOfKind",
          ConnectionProblem::PickNeeded => "notReadyHere.connectionPickNeeded",
        };
        named(key, "profile").with("cli", cli_name(*cli).to_owned())
      }
      Missing::McpOff { .. } => named("notReadyHere.mcpOff", "server"),
      Missing::McpProgram {
        problem: ProgramProblem::NotSet,
        ..
      } => named("notReadyHere.mcpProgramNotSet", "server"),
      Missing::McpProgram {
        problem: ProgramProblem::FileGone(path),
        ..
      } => named("notReadyHere.mcpProgramFileGone", "server").with("path", path.display().to_string()),
      Missing::Secret { .. } => named("notReadyHere.secret", "secret"),
      Missing::LocalMcpProgram {
        problem: LocalProgramProblem::NotFound,
        ..
      } => named("notReadyHere.localMcpProgramNotFound", "server"),
      Missing::LocalMcpProgram {
        problem: LocalProgramProblem::Moved,
        ..
      } => named("notReadyHere.localMcpProgramMoved", "server"),
      Missing::LocalSecret { .. } => named("notReadyHere.localSecret", "secret"),
    }
  }

  fn to_json(&self) -> Value {
    let mut part = serde_json::to_value(&self.missing).expect("a missing part is names and ids, which serialize");
    if let Value::Object(fields) = &mut part {
      fields.insert("name".to_owned(), Value::String(self.name.clone()));
    }
    part
  }
}

/// A resource whose standing cannot be known on this computer, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndecidedPart {
  pub why: Undecided,
  /// The profile, tool or pipeline it is about.
  pub name: String,
}

impl UndecidedPart {
  pub fn message(&self) -> Message {
    let resource = self.name.clone();
    let (key, server) = match &self.why {
      Undecided::NotTeamForm => ("notReadyHere.undecidedNotTeamForm", None),
      Undecided::CannotBePublished(_) => ("notReadyHere.undecidedCannotBePublished", None),
      Undecided::ChoosesMcpServer(server) => ("notReadyHere.undecidedChoosesMcpServer", Some(server.to_string())),
      Undecided::RunsUnknown(_) => ("notReadyHere.undecidedRunsUnknown", None),
      Undecided::ConnectionGone(_) => ("notReadyHere.undecidedConnectionGone", None),
      Undecided::TeamToolSecretById(_) => ("notReadyHere.undecidedTeamToolSecretById", None),
      Undecided::SecretGone(_) => ("notReadyHere.undecidedSecretGone", None),
      Undecided::SecretUnreadable(_) => ("notReadyHere.undecidedSecretUnreadable", None),
      Undecided::Member { why, .. } => {
        return Self {
          why: (**why).clone(),
          name: resource,
        }
        .message();
      }
    };
    let message = Message::new(key).with("resource", resource);
    match server {
      Some(server) => message.with("server", server),
      None => message,
    }
  }

  /// A stable word for why, for a client to match on.
  fn code(&self) -> &'static str {
    match &self.why {
      Undecided::NotTeamForm => "not_team_form",
      Undecided::CannotBePublished(_) => "cannot_be_published",
      Undecided::ChoosesMcpServer(_) => "chooses_mcp_server",
      Undecided::RunsUnknown(_) => "runs_unknown",
      Undecided::ConnectionGone(_) => "connection_gone",
      Undecided::TeamToolSecretById(_) => "team_tool_secret_by_id",
      Undecided::SecretGone(_) => "secret_gone",
      Undecided::SecretUnreadable(_) => "secret_unreadable",
      Undecided::Member { why, .. } => Self {
        why: (**why).clone(),
        name: String::new(),
      }
      .code(),
    }
  }
}

/// One reason a pipeline does not start here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotReadyPart {
  Missing(MissingPart),
  Undecided(UndecidedPart),
}

impl NotReadyPart {
  pub fn message(&self) -> Message {
    match self {
      Self::Missing(part) => part.message(),
      Self::Undecided(part) => part.message(),
    }
  }

  fn to_json(&self) -> Value {
    match self {
      Self::Missing(part) => part.to_json(),
      Self::Undecided(part) => json!({ "kind": "undecided", "why": part.code(), "name": part.name }),
    }
  }
}

/// A pipeline that does not start on this computer, and the parts that keep it from starting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotReadyHere {
  pipeline: String,
  parts: Vec<NotReadyPart>,
}

impl NotReadyHere {
  /// `pipeline` as `readiness` has it, when it is not ready; `None` when it is.
  pub fn of(readiness: &ProjectReadiness, pipeline: &PipelineReadiness) -> Option<Self> {
    let name_of = |resource: ResourceId| {
      readiness
        .resource(resource)
        .map_or_else(|| resource.to_string(), |found| found.name().to_owned())
    };
    let parts = match pipeline.standing() {
      super::ReadinessStanding::Ready => return None,
      super::ReadinessStanding::Missing(_) => pipeline
        .standing()
        .missing()
        .iter()
        .map(|missing| NotReadyPart::Missing(MissingPart::new(missing.clone(), &name_of)))
        .collect(),
      super::ReadinessStanding::Undecided(why) => {
        vec![NotReadyPart::Undecided(undecided(why, &name_of, pipeline.name()))]
      }
    };
    Some(Self {
      pipeline: pipeline.name().to_owned(),
      parts,
    })
  }

  /// `pipeline` (by name), whose pinned versions lack `missing`, which is not empty.
  pub fn of_missing(pipeline: impl Into<String>, missing: Vec<MissingPart>) -> Self {
    Self {
      pipeline: pipeline.into(),
      parts: missing.into_iter().map(NotReadyPart::Missing).collect(),
    }
  }

  pub fn pipeline(&self) -> &str {
    &self.pipeline
  }

  pub fn parts(&self) -> &[NotReadyPart] {
    &self.parts
  }

  /// Why nothing started, then each part with its fix.
  pub fn messages(&self) -> Vec<Message> {
    std::iter::once(Message::new("notReadyHere.refused").with("pipeline", self.pipeline.clone()))
      .chain(self.parts.iter().map(NotReadyPart::message))
      .collect()
  }

  /// The parts as a client reads them: a kind with the ids and names it is about, never a value.
  pub fn parts_json(&self) -> Vec<Value> {
    self.parts.iter().map(NotReadyPart::to_json).collect()
  }
}

/// The reason `why` leaves a resource undecided, with the resource it is about: a pipeline names the member that is.
fn undecided(why: &Undecided, name_of: &dyn Fn(ResourceId) -> String, pipeline: &str) -> UndecidedPart {
  match why {
    Undecided::Member { id, why } => UndecidedPart {
      why: (**why).clone(),
      name: name_of(*id),
    },
    Undecided::RunsUnknown(id) => UndecidedPart {
      why: why.clone(),
      name: name_of(*id),
    },
    other => UndecidedPart {
      why: other.clone(),
      name: pipeline.to_owned(),
    },
  }
}

/// A CLI by the name of its program, or of its API; an identifier, not a sentence.
const fn cli_name(cli: orchester_core::connection::AgentProvider) -> &'static str {
  use orchester_core::connection::AgentProvider;
  match cli {
    AgentProvider::CodexCli => "codex",
    AgentProvider::ClaudeCodeCli => "claude",
    AgentProvider::AnthropicApi => "anthropic_api",
    AgentProvider::OpenAiApi => "openai_api",
  }
}
