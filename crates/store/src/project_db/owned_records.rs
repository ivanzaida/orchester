//! Rows the desktop owns, which the runner's executor asks its coordinator to write
//! (`docs/architecture/remote-runners.md`, rule 3): the rules of Approve similar, Always allow and Approve for every
//! run of this project, and how many requests each answered (`approval_rules`, `project_approval_rules`); Always
//! allow's new profile version and draft (`resource_versions`, `resource_drafts`) and an MCP server's tools that run
//! without asking (`mcp_servers`); the CLI version a connection's model needs (`connections`); and agents' memory
//! records (`memories`). Each request is a value that can be sent, and [`ProjectDb`] applies it with the store
//! functions and in the transactions the executor used before, answering what the executor needs to go on: the new
//! rule's id, the kept record's id and status, or that Always allow is not available.
//!
//! A memory request carries content the executor screened already against the secret values of the computer it runs
//! on ([`Handoff::screen`], [`MemoryNote::screen`]); the coordinator reads the project's memory setting and the
//! records it changes in one transaction, so a stale write is refused as before.

use orchester_core::{
  id::{ConnectionId, MemoryId, ResourceId, RunId, TaskId},
  memory::{
    AgentProvenance, ContainsSecret, Handoff, HandoffWrite, Memory, MemoryFilter, MemoryNote, MemoryScope,
    MemoryStatus, MemoryStore, NoteWrite, ScopeFilter, SecretScreen, StepFate,
  },
  permission::{CommandPrefix, category::ClassifiedCommand},
  resource::ModelId,
  run::{ApprovalRuleId, ProjectRuleId, ProjectScope, RuleScope, allowing_tool},
  store::{ConnectionStore, McpServerStore},
  time::Timestamp,
};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::StoreError;

/// A rule a request's answer makes, or a request a rule of the project answered. A request a rule of the run answered
/// is recorded with the request itself (`ApprovalRecord::rule`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RuleRecord {
  /// A rule of `run` ([`ProjectDb::add_approval_rule`]): Approve similar, or a check approved once per run.
  Run {
    run: RunId,
    scope: RuleScope,
    at: Timestamp,
  },
  /// Approve for every run of this project, given to request `request` of `run`
  /// ([`ProjectDb::add_project_approval_rule`]).
  Project {
    scope: ProjectScope,
    run: RunId,
    request: String,
    at: Timestamp,
  },
  /// Rule `rule` of the project answered a request ([`ProjectDb::project_rule_answered`]).
  ProjectAnswered { rule: ProjectRuleId },
}

/// What a [`RuleRecord`] did: the rule that answers from now on (an active one of the same scope when there was one
/// already), or that a project rule's count went up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "rule")]
pub enum RuleAnswer {
  Run(ApprovalRuleId),
  Project(ProjectRuleId),
  Counted,
}

/// Always allow, as a person answered a request with it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AlwaysAllowRecord {
  /// `prefix` of `command`, published in a new version of agent profile `profile`
  /// ([`ProjectDb::publish_allowed_command`]) and kept by `run` as a rule, so the run keeps allowing it.
  Command {
    run: RunId,
    profile: ResourceId,
    command: ClassifiedCommand,
    prefix: CommandPrefix,
    at: Timestamp,
  },
  /// Tool `tool` of MCP server `server`, listed among its tools that run without asking.
  McpTool { server: String, tool: String },
}

/// Whether Always allow was left as asked, or is not available for the request: Always allow does not offer the
/// prefix, the profile has no list to add to, the server is gone or its list is full.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlwaysAllowAnswer {
  Allowed,
  Unavailable,
}

/// The CLI version model `model` needs, recorded on connection `connection`
/// ([`ConnectionStore::record_model_requirement`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRequirement {
  connection: ConnectionId,
  model: ModelId,
  required: String,
}

impl ModelRequirement {
  pub fn new(connection: ConnectionId, model: &ModelId, required: &str) -> Self {
    Self {
      connection,
      model: model.clone(),
      required: required.to_owned(),
    }
  }
}

/// A memory write of an agent, its content screened where it was sent from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MemoryRecord {
  /// The handoff of a step that ended as `fate`, from transcript entries `sources` ([`Handoff::changes`]).
  Handoff {
    handoff: Handoff,
    task: TaskId,
    provenance: AgentProvenance,
    sources: Vec<u64>,
    fate: StepFate,
    at: Timestamp,
  },
  /// A `save_memory` call's note ([`MemoryNote::changes`]).
  Note {
    note: MemoryNote,
    task: TaskId,
    provenance: AgentProvenance,
    at: Timestamp,
  },
}

/// What a [`MemoryRecord`] kept: the handoff, or the note's record with where it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MemoryAnswer {
  Handoff,
  Note { memory: MemoryId, status: MemoryStatus },
}

impl ProjectDb {
  /// Applies `record` as its direct write does.
  pub fn write_rule(&mut self, record: &RuleRecord) -> Result<RuleAnswer, StoreError> {
    Ok(match record {
      RuleRecord::Run { run, scope, at } => RuleAnswer::Run(self.add_approval_rule(*run, scope, *at)?.id),
      RuleRecord::Project {
        scope,
        run,
        request,
        at,
      } => RuleAnswer::Project(self.add_project_approval_rule(scope, (*run, request), *at)?.id),
      RuleRecord::ProjectAnswered { rule } => {
        self.project_rule_answered(*rule)?;
        RuleAnswer::Counted
      }
    })
  }

  /// Leaves what Always allow promises: a command's prefix published in its profile and kept as a rule of the run,
  /// or a tool listed on its server.
  pub fn write_always_allow(&mut self, record: &AlwaysAllowRecord) -> Result<AlwaysAllowAnswer, StoreError> {
    match record {
      AlwaysAllowRecord::Command {
        run,
        profile,
        command,
        prefix,
        at,
      } => {
        match self.publish_allowed_command(*profile, command, prefix, *at) {
          Ok(_) => {}
          Err(StoreError::AlwaysAllow(_)) => return Ok(AlwaysAllowAnswer::Unavailable),
          Err(error) => return Err(error),
        }
        let scope = RuleScope::AlwaysCommand { prefix: prefix.clone() };
        self.add_approval_rule(*run, &scope, *at)?;
      }
      AlwaysAllowRecord::McpTool { server, tool } => {
        let mut servers = self.mcp_servers()?;
        let Some(stored) = servers.iter_mut().find(|stored| stored.name().as_str() == server) else {
          return Ok(AlwaysAllowAnswer::Unavailable);
        };
        let Ok(tools) = allowing_tool(stored.exempt_tools(), tool) else {
          return Ok(AlwaysAllowAnswer::Unavailable);
        };
        stored.set_exempt_tools(tools);
        self.save_mcp_server(stored)?;
      }
    }
    Ok(AlwaysAllowAnswer::Allowed)
  }

  /// Records `requirement` on its connection; [`StoreError::ConnectionNotFound`] when it was removed.
  pub fn write_model_requirement(&mut self, requirement: &ModelRequirement) -> Result<(), StoreError> {
    self.record_model_requirement(requirement.connection, &requirement.model, &requirement.required)
  }

  /// Keeps `record` under the project's memory setting as it is now, reading the records it changes and storing the
  /// changes in one transaction.
  pub fn write_memory(&mut self, record: &MemoryRecord) -> Result<MemoryAnswer, StoreError> {
    let settings = self.memory_settings()?;
    // The content was screened where it was sent from, against the secret values of that computer.
    let screen = SecretScreen::new(Vec::new());
    let screened = "content screened before it was sent holds no secret value to screen here";
    match record {
      MemoryRecord::Handoff {
        handoff,
        task,
        provenance,
        sources,
        fate,
        at,
      } => {
        let write = HandoffWrite {
          task: *task,
          provenance,
          sources,
          fate: *fate,
          settings: &settings,
          screen: &screen,
          now: *at,
        };
        self
          .change_memories(&filter(MemoryScope::Task(*task)), |earlier| {
            handoff.changes(&write, earlier)
          })?
          .expect(screened);
        Ok(MemoryAnswer::Handoff)
      }
      MemoryRecord::Note {
        note,
        task,
        provenance,
        at,
      } => {
        let write = NoteWrite {
          task: *task,
          provenance,
          settings: &settings,
          screen: &screen,
          now: *at,
        };
        let mut kept: Option<Memory> = None;
        self
          .change_memories(&filter(note.scope(*task)), |earlier| -> Result<_, ContainsSecret> {
            let (changes, record) = note.changes(&write, &earlier)?;
            kept = Some(record);
            Ok(changes)
          })?
          .expect(screened);
        let kept = kept.expect("a kept note's record is set when its changes are made");
        Ok(MemoryAnswer::Note {
          memory: kept.id(),
          status: kept.status(),
        })
      }
    }
  }
}

/// Every record of `scope`, whatever its status.
const fn filter(scope: MemoryScope) -> MemoryFilter {
  MemoryFilter {
    scope: match scope {
      MemoryScope::Task(task) => ScopeFilter::Task(task),
      MemoryScope::Project => ScopeFilter::Project,
    },
    status: None,
    pinned_only: false,
  }
}
