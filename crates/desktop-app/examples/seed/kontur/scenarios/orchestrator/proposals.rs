//! The decision cards that wait for the person in *Every card* (`3 · Decision cards` `bxJZp`), on the scenario's own
//! tasks so each card reads its facts: a start of a draft on Feature delivery, a merge of the run ready to merge, a
//! cancel of the run whose plan waits, a publication of a profile draft that turns network access on, which widens
//! what its agents may do, and an MCP server to add (`figma`), whose facts go on past the three a card shows before
//! *Show details*.

use orchester_core::{
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, ProposedChange, Publishing},
    mcp::{McpChange, McpOperation, McpServerShown, McpTransportShown, McpVariableShown},
    proposal::{ProposedStart, StartBasis, brief_excerpt, brief_is_cut},
  },
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, Draft, Pipeline, RepositoryAccess, ResourceKind},
  run::Run,
  store::ResourceStore,
  task::Task,
  time::Timestamp,
};

use super::live::Ran;
use crate::kontur::{Kontur, SeedError};

/// A start of draft `task` on its pipeline's latest published version, as the chat proposes it.
pub fn start(kontur: &Kontur, task: &Task) -> Result<ChatAction, SeedError> {
  let details = task.details();
  let pipeline = details.pipeline.ok_or(SeedError::NoPipeline)?;
  let name = kontur
    .db
    .resources(ResourceKind::Pipeline)?
    .into_iter()
    .find(|resource| resource.id() == pipeline)
    .ok_or(SeedError::NoPipeline)?
    .name()
    .as_str()
    .to_owned();
  let version = kontur
    .db
    .latest_version::<Pipeline>(pipeline)?
    .map(|version| version.number().get());
  let start = ProposedStart {
    pipeline: name,
    title: details.title.as_str().to_owned(),
    brief: brief_excerpt(&details.brief),
    brief_cut: brief_is_cut(&details.brief),
    criteria: details.acceptance_criteria.clone(),
    basis: StartBasis {
      revision: task.revision().get(),
      pipeline: Some(pipeline),
      version,
    },
  };
  Ok(ChatAction::new(
    "run_task",
    Some(task.key().to_string()),
    ActionOutcome::Proposed(Box::new(start)),
  ))
}

/// A merge of `ready`'s run, which succeeded, into the branch it started from.
pub fn merge(ready: &Ran) -> ChatAction {
  let run = &ready.run;
  proposed(
    ("merge_run", ready),
    Change::Merge {
      run: run.id(),
      number: run.number().get(),
      checkpoint: run
        .last_checkpoint()
        .map(|checkpoint| checkpoint.revision().to_string()),
      into: run.base_branch().map(|branch| branch.as_str().to_owned()),
    },
  )
}

/// A cancel of `going`'s run.
pub fn cancel(going: &Ran) -> ChatAction {
  let run: &Run = &going.run;
  proposed(
    ("cancel_run", going),
    Change::Cancel {
      run: run.id(),
      number: run.number().get(),
    },
  )
}

/// The publication of a draft of the first published profile with network access on, saved at `saved`.
pub fn publish_widening(kontur: &mut Kontur, saved: Timestamp) -> Result<ChatAction, SeedError> {
  let (resource, name) = kontur
    .db
    .resources(ResourceKind::AgentProfile)?
    .into_iter()
    .map(|found| (found.id(), found.name().as_str().to_owned()))
    .next()
    .ok_or_else(|| SeedError::Invalid("the catalogue publishes a profile".to_owned()))?;
  let latest = kontur
    .db
    .latest_version::<AgentProfile>(resource)?
    .ok_or_else(|| SeedError::Invalid("the profile is published".to_owned()))?;
  let draft = latest.body().clone().with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  ));
  let number = latest.number();
  kontur
    .db
    .update_draft::<AgentProfile>(resource, |_, _| Ok(Draft::new(draft, Some(number), saved)))?;
  Ok(ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: name,
      change: Change::Publish {
        of: Publishing::Profile,
        resource,
        version: number.get() + 1,
        saved,
      },
    })),
    ..ChatAction::new(Publishing::Profile.tool(), None, ActionOutcome::Waits)
  })
}

/// Adding the MCP server `figma`, a program the agents of Designer and Reviewer would start with a secret in its
/// variable: it widens what they may do, so only the person applies it.
pub fn add_mcp_server() -> ChatAction {
  let server = McpServerShown {
    name: "figma".to_owned(),
    transport: McpTransportShown::Stdio {
      program: "npx".to_owned(),
      found: Some("C:\\Program Files\\nodejs\\npx.cmd".to_owned()),
      args: vec!["figma-developer-mcp".to_owned(), "--stdio".to_owned()],
      working_directory: None,
    },
    environment: vec![McpVariableShown {
      name: "FIGMA_TOKEN".to_owned(),
      secret: Some("FIGMA_TOKEN@development".to_owned()),
    }],
    exempt_tools: Vec::new(),
    enabled: true,
    used_by: vec!["Designer".to_owned(), "Reviewer".to_owned()],
  };
  let change = McpChange {
    operation: McpOperation::Add,
    id: None,
    server,
    widens: true,
    basis: String::new(),
    arguments: serde_json::Map::new(),
  };
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "figma".to_owned(),
      change: Change::Mcp(Box::new(change)),
    })),
    ..ChatAction::new(McpOperation::Add.tool(), None, ActionOutcome::Waits)
  }
}

/// Tool `tool`'s proposal of `change` on `ran`'s task, waiting for the person.
fn proposed((tool, ran): (&str, &Ran), change: Change) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: ran.task.details().title.as_str().to_owned(),
      change,
    })),
    ..ChatAction::new(tool, Some(ran.task.key().to_string()), ActionOutcome::Waits)
  }
}
