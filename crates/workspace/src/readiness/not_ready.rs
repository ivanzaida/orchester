//! The answer of a start that does not start because its pipeline is not ready on this computer (`not_ready_here`,
//! `docs/design/team-configuration.md`, "What a computer fills in"): the parts that are missing or cannot be known,
//! each with a message that says the fix. Shared by every start: a team start, and a local one that refuses the same
//! way. A part holds names and ids only: no secret value, and no path but the program path the person set themselves
//! for a team MCP server whose file is gone, which they need to see to fix it.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
  sharing::{ConnectionProblem, LocalProgramProblem, McpUnsupported, Missing, ProgramProblem},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{PipelineReadiness, ProjectReadiness, ReadinessStanding, ResourceReadiness, Undecided};
use crate::{messages::Message, team_work::ClaimEnd};

/// One part a pipeline lacks, with the name of the profile, reviewer or MCP server it is about (the secret's own name
/// for a secret). Kept with a start that failed on it, so it is serializable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingPart {
  pub missing: Missing,
  pub name: String,
  /// For a secret, the profile, MCP server or tool of the project that needs it and lacks its value, which is where
  /// the person sets it; `None` for any other part, and for a part found only in the versions a team server pinned.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner: Option<ResourceId>,
}

impl MissingPart {
  /// `missing`, its resource named by `name_of`.
  pub fn new(missing: Missing, name_of: &dyn Fn(ResourceId) -> String) -> Self {
    let name = match &missing {
      Missing::Connection { profile, .. } => name_of(*profile),
      Missing::DecisionConnection { reviewer, .. } => name_of(*reviewer),
      Missing::McpOff { server } | Missing::McpUnsupported { server, .. } | Missing::McpProgram { server, .. } => {
        name_of(*server)
      }
      Missing::Secret { name } | Missing::SecretUnreadable { name } | Missing::LocalSecret { name, .. } => {
        name.as_str().to_owned()
      }
      Missing::LocalMcpProgram { name, .. } => name.as_str().to_owned(),
    };
    Self {
      missing,
      name,
      owner: None,
    }
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
      Missing::DecisionConnection { problem, .. } => {
        let key = match problem {
          ConnectionProblem::NoneOfKind => "notReadyHere.decisionConnectionNoneOfKind",
          ConnectionProblem::PickNeeded => "notReadyHere.decisionConnectionPickNeeded",
        };
        named(key, "reviewer")
      }
      Missing::McpOff { .. } => named("notReadyHere.mcpOff", "server"),
      Missing::McpUnsupported {
        why: McpUnsupported::MoreThanOneSecretHeader,
        ..
      } => named("notReadyHere.mcpMoreThanOneSecretHeader", "server"),
      Missing::McpProgram {
        problem: ProgramProblem::NotSet,
        ..
      } => named("notReadyHere.mcpProgramNotSet", "server"),
      Missing::McpProgram {
        problem: ProgramProblem::FileGone(path),
        ..
      } => named("notReadyHere.mcpProgramFileGone", "server").with("path", path.display().to_string()),
      Missing::Secret { .. } => named("notReadyHere.secret", "secret"),
      Missing::SecretUnreadable { .. } => named("notReadyHere.secretUnreadable", "secret"),
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
    undecided_message(&self.why, &self.name)
  }

  /// A stable word for why, for a client to match on.
  fn code(&self) -> &'static str {
    undecided_code(&self.why)
  }
}

/// What to do about `why`, naming `resource`; a pipeline's member says what the member is undecided for.
fn undecided_message(why: &Undecided, resource: &str) -> Message {
  let (key, extra) = match why {
    Undecided::NotTeamForm => ("notReadyHere.undecidedNotTeamForm", None),
    Undecided::CannotBePublished(_) => ("notReadyHere.undecidedCannotBePublished", None),
    Undecided::ChoosesMcpServer(server) => ("notReadyHere.undecidedChoosesMcpServer", Some(server.to_string())),
    Undecided::ChoosesArchivedMcpServer(_, name) => {
      ("notReadyHere.undecidedChoosesArchivedMcpServer", Some(name.clone()))
    }
    Undecided::UsesArchivedTool(_, name) => ("notReadyHere.undecidedUsesArchivedTool", Some(name.clone())),
    Undecided::UsesArchivedInstruction(_, name) => {
      ("notReadyHere.undecidedUsesArchivedInstruction", Some(name.clone()))
    }
    Undecided::RunsUnknown(_) => ("notReadyHere.undecidedRunsUnknown", None),
    Undecided::RunsArchived(_, name) => ("notReadyHere.undecidedRunsArchived", Some(name.clone())),
    Undecided::ChoosesArchivedLocalMcpServer(_, name) => (
      "notReadyHere.undecidedChoosesArchivedLocalMcpServer",
      Some(name.clone()),
    ),
    Undecided::ConnectionGone(_) => ("notReadyHere.undecidedConnectionGone", None),
    Undecided::TeamToolSecretById(_) => ("notReadyHere.undecidedTeamToolSecretById", None),
    Undecided::SecretGone(_) => ("notReadyHere.undecidedSecretGone", None),
    Undecided::SecretUnreadable(_) => ("notReadyHere.undecidedSecretUnreadable", None),
    Undecided::Member { why, .. } => return undecided_message(why, resource),
  };
  let message = Message::new(key).with("resource", resource.to_owned());
  match (why, extra) {
    (Undecided::ChoosesMcpServer(_), Some(server)) => message.with("server", server),
    (_, Some(name)) => message.with("name", name),
    (_, None) => message,
  }
}

fn undecided_code(why: &Undecided) -> &'static str {
  match why {
    Undecided::NotTeamForm => "not_team_form",
    Undecided::CannotBePublished(_) => "cannot_be_published",
    Undecided::ChoosesMcpServer(_) => "chooses_mcp_server",
    Undecided::ChoosesArchivedMcpServer(..) => "chooses_archived_mcp_server",
    Undecided::UsesArchivedTool(..) => "uses_archived_tool",
    Undecided::UsesArchivedInstruction(..) => "uses_archived_instruction",
    Undecided::RunsUnknown(_) => "runs_unknown",
    Undecided::RunsArchived(..) => "runs_archived",
    Undecided::ChoosesArchivedLocalMcpServer(..) => "chooses_archived_mcp_server",
    Undecided::ConnectionGone(_) => "connection_gone",
    Undecided::TeamToolSecretById(_) => "team_tool_secret_by_id",
    Undecided::SecretGone(_) => "secret_gone",
    Undecided::SecretUnreadable(_) => "secret_unreadable",
    Undecided::Member { why, .. } => undecided_code(why),
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

/// When a start found that its pipeline is not ready here: what the first lines of its answer may say happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
  /// Before the team server was asked for anything: no claim, no work, no attempt.
  BeforeClaim,
  /// After `work.start`, from the versions the server pinned: the server made the work, nothing ran, and the claim
  /// was let go of, kept for another run, or could not be.
  AfterStart(ClaimEnd),
}

/// A pipeline that does not start on this computer, and the parts that keep it from starting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotReadyHere {
  pipeline: String,
  /// The version this computer uses, when the answer is read from its readiness; the team server's pin is not.
  number: Option<VersionNumber>,
  parts: Vec<NotReadyPart>,
  found: Found,
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
      ReadinessStanding::Ready => return None,
      ReadinessStanding::Missing(_) => pipeline
        .standing()
        .missing()
        .iter()
        .map(|missing| {
          let mut part = MissingPart::new(missing.clone(), &name_of);
          part.owner = secret_owner(readiness, missing);
          NotReadyPart::Missing(part)
        })
        .collect(),
      ReadinessStanding::Undecided(why) => {
        vec![NotReadyPart::Undecided(undecided(why, &name_of, pipeline.name()))]
      }
    };
    Some(Self {
      pipeline: pipeline.name().to_owned(),
      number: Some(pipeline.number()),
      parts,
      found: Found::BeforeClaim,
    })
  }

  /// `pipeline` (by name), whose versions the team server pinned for a start it accepted lack `missing`, which is not
  /// empty: the server made the work, though nothing ran; `claim` says what became of the task's claim.
  pub fn after_start(pipeline: impl Into<String>, missing: Vec<MissingPart>, claim: ClaimEnd) -> Self {
    Self {
      pipeline: pipeline.into(),
      number: None,
      parts: missing.into_iter().map(NotReadyPart::Missing).collect(),
      found: Found::AfterStart(claim),
    }
  }

  pub fn pipeline(&self) -> &str {
    &self.pipeline
  }

  /// The version of the pipeline this computer uses; none for a refusal found in the versions the team server pinned.
  pub const fn number(&self) -> Option<VersionNumber> {
    self.number
  }

  pub fn parts(&self) -> &[NotReadyPart] {
    &self.parts
  }

  /// Why nothing started, then each part with its fix.
  pub fn messages(&self) -> Vec<Message> {
    let lead = match self.found {
      Found::BeforeClaim => "notReadyHere.refused",
      Found::AfterStart(ClaimEnd::Released) => "notReadyHere.refusedAfterStart",
      Found::AfterStart(ClaimEnd::Kept) => "notReadyHere.refusedAfterStartKept",
      Found::AfterStart(ClaimEnd::Held) => "notReadyHere.refusedAfterStartHeld",
    };
    std::iter::once(Message::new(lead).with("pipeline", self.pipeline.clone()))
      .chain(self.parts.iter().map(NotReadyPart::message))
      .collect()
  }

  /// Why nothing started in one line, for a place that has no room for the parts; the parts are in [`Self::parts`].
  pub fn short(&self) -> Message {
    Message::new("notReadyHere.refusedShort").with("pipeline", self.pipeline.clone())
  }

  /// What became of the task's claim when the start was found not ready after the team server accepted it
  /// ([`Self::after_start`]); `None` for a refusal before any claim.
  pub const fn claim_end(&self) -> Option<ClaimEnd> {
    match self.found {
      Found::AfterStart(claim) => Some(claim),
      Found::BeforeClaim => None,
    }
  }

  /// The team server made the work, though nothing ran: the start was found not ready after it was accepted.
  pub const fn after_server_started(&self) -> bool {
    self.claim_end().is_some()
  }

  /// The parts as a client reads them: a kind with the ids and names it is about, never a value.
  pub fn parts_json(&self) -> Vec<Value> {
    self.parts.iter().map(NotReadyPart::to_json).collect()
  }
}

/// The resource of `readiness` that lacks the secret `missing` is, where the person sets the value: the MCP server or
/// tool that needs it, and a profile only when no server or tool does (a profile lacks the secret of the servers it
/// chooses, which the server itself shows). `None` for any other part.
fn secret_owner(readiness: &ProjectReadiness, missing: &Missing) -> Option<ResourceId> {
  if !matches!(
    missing,
    Missing::Secret { .. } | Missing::SecretUnreadable { .. } | Missing::LocalSecret { .. }
  ) {
    return None;
  }
  let lacking = || {
    readiness
      .resources()
      .iter()
      .filter(|found| found.standing().missing().contains(missing))
  };
  lacking()
    .find(|found| matches!(found.kind(), ResourceKind::McpServer | ResourceKind::Tool))
    .or_else(|| lacking().next())
    .map(ResourceReadiness::resource)
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
const fn cli_name(cli: AgentProvider) -> &'static str {
  match cli {
    AgentProvider::CodexCli => "codex",
    AgentProvider::ClaudeCodeCli => "claude",
    AgentProvider::AnthropicApi => "anthropic_api",
    AgentProvider::OpenAiApi => "openai_api",
  }
}
