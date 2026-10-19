//! Publishing a draft, canceling a run, merging one, answering an agent's question, sending a run's agents a message
//! and deciding how a run that waits for a person goes on wait for the person when the Orchestrator's agent asks for
//! them (`docs/design/orchestrator-toolset.md`, "Decisions" 3 to 5), unless the person's switch lets the chat do it
//! itself: *Let the Orchestrator publish drafts*, *merge runs* or *answer agents* (none lets it continue a run). The
//! agent's call checks what it names and changes nothing: the conversation keeps the proposed change and the agent is
//! told it waits. The person applies it through the same tool any client of the app calls, as their own decision (what
//! it widens is theirs to decide, as in the window), or dismisses it ([`super::proposal::decide`]); a draft saved again
//! or published meanwhile, or a run with a newer checkpoint, is refused and needs a fresh proposal. An answer or a
//! message is sent as the chat's, recorded as automation's.

mod answer;
mod continuation;

use std::path::Path;

use orchester_core::{
  id::{ResourceId, RunId},
  orchestrator::change::{Change, ProposedChange, Publishing},
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, Tool},
  revision::Revision,
  run::Run,
  store::{ResourceStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::{
  tools::{ANSWER_QUESTION, CANCEL_RUN, CONTINUE_RUN, ChatScope, MERGE_RUN, SEND_MESSAGE, app_tool},
  widening,
};
use crate::{
  app_mcp::{
    Missing, PersonDecision, Services, ToolError, Translate, answer_for_orchestrator, blocking, find_run,
    setup::merge_proposed,
  },
  display,
  messages::Message,
};

/// What the agent's call of tool `name` with `arguments` in `scope` publishes, cancels, merges, answers, sends or
/// continues, and the key of the task it names; a refusal when there is nothing to change.
pub(super) async fn subject(
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<(ProposedChange, Option<String>), ToolError> {
  let text = |argument: &str| {
    arguments
      .get(argument)
      .and_then(Value::as_str)
      .map(str::to_owned)
      .ok_or_else(|| ToolError::InvalidArguments(format!("{argument}: required")))
  };
  let root = scope.root.clone();
  let (change, task) = match (Publishing::of_tool(name), name) {
    (Some(kind), _) => {
      let named = text(kind.argument())?;
      (blocking(move || publication(&root, kind, &named)).await?, None)
    }
    (None, ANSWER_QUESTION) => {
      let asked = answer::Asked::of(arguments)?;
      blocking(move || answer::answer_change(&root, asked)).await?
    }
    (None, SEND_MESSAGE) => {
      let sending = answer::Sending::of(arguments)?;
      blocking(move || answer::message_change(&root, sending)).await?
    }
    (None, CONTINUE_RUN) => {
      let arguments = arguments.clone();
      blocking(move || continuation::continue_change(&root, &arguments)).await?
    }
    (None, _) => {
      let (named, cancel) = (text("run")?, name == CANCEL_RUN);
      blocking(move || run_change(&root, cancel, &named)).await?
    }
  };
  Ok((change, task))
}

/// What publishing `change`'s draft in `scope` would widen in what agents may do; nothing for a cancel or merge.
pub(super) async fn widens(scope: &ChatScope, change: &ProposedChange) -> Result<Vec<Message>, ToolError> {
  let Change::Publish { of, resource, .. } = change.change else {
    return Ok(Vec::new());
  };
  let root = scope.root.clone();
  blocking(move || Ok(widening::of_draft(&ProjectDb::open(&root)?, of, resource)?)).await
}

/// Applies `change`, proposed in `scope`, through the tool that carries it out, as the person's own decision:
/// refused as for any client but for what widens what agents may do, which the person decided when it is exactly
/// what they were shown (`shown`); refused when what it names changed since it was proposed. A merge merges only the
/// commit that was proposed.
pub(super) async fn apply(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  change: &ProposedChange,
  shown: &[Message],
) -> Result<Value, ToolError> {
  let root = scope.root.clone();
  let project = scope.project.to_string();
  match change.change.clone() {
    Change::Publish {
      of,
      resource,
      version,
      saved,
    } => {
      let (subject, widens) = (change.subject.clone(), shown.to_vec());
      // Told early, by name; the tool checks the draft and what it widens again where it publishes.
      blocking(move || unchanged_draft(&root, (of, resource), ((saved, version), &widens), subject)).await?;
      let person = services.clone().decided_by_person(PersonDecision {
        saved,
        version,
        widens: shown.to_vec(),
      });
      let (tool, arguments) = tool_arguments(scope, change);
      app_tool((&person, translate), tool, arguments).await
    }
    Change::Cancel { .. } => {
      let (tool, arguments) = tool_arguments(scope, change);
      app_tool((services, translate), tool, arguments).await
    }
    Change::Merge {
      run,
      number,
      checkpoint,
      ..
    } => {
      let Some(checkpoint) = checkpoint else {
        return Err(no_checkpoint(number));
      };
      let expected = Revision::new(&checkpoint).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
      blocking(move || unchanged_run(&root, (run, number), Some(&checkpoint))).await?;
      merge_proposed(services, translate, (project, run.to_string()), expected).await
    }
    // The tool checks again that the choice is offered where it continues the run.
    Change::Continue {
      run, number, attempt, ..
    } => {
      blocking(move || continuation::unchanged(&root, (&run.to_string(), number), attempt)).await?;
      let (tool, arguments) = tool_arguments(scope, change);
      app_tool((services, translate), tool, arguments).await
    }
    // The tool checks the question, or the run's state, again where it sends: one answered or changed since is refused.
    Change::Answer { .. } | Change::Message { .. } => {
      let (tool, arguments) = tool_arguments(scope, change);
      answer_for_orchestrator(services, translate, tool, arguments).await
    }
  }
}

/// The tool that carries out `change` in `scope`, and its arguments: the chat's project and what the change acts on,
/// by its id, with an answer's or a message's words.
pub(super) fn tool_arguments(scope: &ChatScope, change: &ProposedChange) -> (&'static str, JsonObject) {
  let (tool, argument, named) = match &change.change {
    Change::Publish { of, resource, .. } => (of.tool(), of.argument(), resource.to_string()),
    Change::Cancel { run, .. } => (CANCEL_RUN, "run", run.to_string()),
    Change::Merge { run, .. } => (MERGE_RUN, "run", run.to_string()),
    Change::Answer { run, .. } => (ANSWER_QUESTION, "run", run.to_string()),
    Change::Message { run, .. } => (SEND_MESSAGE, "run", run.to_string()),
    Change::Continue { run, .. } => (CONTINUE_RUN, "run", run.to_string()),
  };
  let mut arguments = JsonObject::new();
  arguments.insert("project".to_owned(), Value::String(scope.project.to_string()));
  arguments.insert(argument.to_owned(), Value::String(named));
  answer::words(&change.change, &mut arguments);
  if let Change::Continue { choice, .. } = &change.change {
    continuation::words(choice, &mut arguments);
  }
  (tool, arguments)
}

/// The publication of the draft `named` of kind `kind` in the project at `root`. Blocks on SQLite.
fn publication(root: &Path, kind: Publishing, named: &str) -> Result<ProposedChange, ToolError> {
  let db = ProjectDb::open(root)?;
  let (resource, subject) = draft_named(&db, kind, named)?;
  let (saved, version) = mark(&db, kind, resource)?
    .ok_or_else(|| ToolError::refused("no_draft", vec![Message::new("appMcp.errors.noDraft")]))?;
  Ok(ProposedChange {
    subject,
    change: Change::Publish {
      of: kind,
      resource,
      version,
      saved,
    },
  })
}

/// The title of `run`'s task, what a change of the run names as its subject, and the task's key.
fn run_subject(db: &ProjectDb, run: &Run) -> Result<(String, Option<String>), ToolError> {
  let task = db.task(run.task())?;
  let subject = task
    .as_ref()
    .map(|task| task.details().title.as_str().to_owned())
    .unwrap_or_default();
  Ok((subject, task.map(|task| task.key().to_string())))
}

/// Canceling (`cancel`) or merging the run `named` in the project at `root`, and the key of its task. A run that
/// ended has nothing to cancel. Blocks on SQLite.
fn run_change(root: &Path, cancel: bool, named: &str) -> Result<(ProposedChange, Option<String>), ToolError> {
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, named)?;
  let (subject, task) = run_subject(&db, &run)?;
  let (id, number) = (run.id(), run.number().get());
  let change = if cancel {
    if run.status().is_terminal() {
      return Err(ToolError::refused(
        "run_ended",
        vec![Message::new("runs.control.ended")],
      ));
    }
    Change::Cancel { run: id, number }
  } else {
    // Only committed work is merged, and only the commit the person is shown.
    if checkpoint(&run).is_none() {
      return Err(no_checkpoint(number));
    }
    Change::Merge {
      run: id,
      number,
      checkpoint: checkpoint(&run),
      into: run.base_branch().map(|branch| branch.as_str().to_owned()),
    }
  };
  Ok((ProposedChange { subject, change }, task))
}

/// Refused unless the draft of `resource` is still the one saved at `saved`, to be published as `version`, and
/// widens what agents may do by `widens`, what the person was shown.
fn unchanged_draft(
  root: &Path,
  (kind, resource): (Publishing, ResourceId),
  (proposed, widens): ((Timestamp, u32), &[Message]),
  subject: String,
) -> Result<(), ToolError> {
  let db = ProjectDb::open(root)?;
  let now = mark(&db, kind, resource)?;
  if now == Some(proposed) && widening::of_draft(&db, kind, resource)? == widens {
    return Ok(());
  }
  // Saved again: when it was. Otherwise it is gone, or what it widens changed with the version it is compared with.
  let reason = match now {
    Some((saved, _)) if saved != proposed.0 => {
      Message::new("orchestrator.proposal.draftChangedAt").with("time", display::clock(saved))
    }
    _ => Message::new("orchestrator.proposal.draftChanged").with("name", subject),
  };
  Err(ToolError::refused("changed_since_proposed", vec![reason]))
}

/// A merge of run `#number`, which has no checkpoint: nothing of its work is committed.
fn no_checkpoint(number: u32) -> ToolError {
  ToolError::refused(
    "no_checkpoint",
    vec![Message::new("orchestrator.proposal.noCheckpoint").with("run", number.to_string())],
  )
}

/// Refused unless run `run` (`#number`) still holds what its last checkpoint `proposed` held.
fn unchanged_run(root: &Path, (run, number): (RunId, u32), proposed: Option<&str>) -> Result<(), ToolError> {
  let run = find_run(&ProjectDb::open(root)?, &run.to_string())?;
  if checkpoint(&run).as_deref() == proposed {
    return Ok(());
  }
  Err(ToolError::refused(
    "changed_since_proposed",
    vec![Message::new("orchestrator.proposal.runChanged").with("run", number.to_string())],
  ))
}

/// The commit of `run`'s last checkpoint.
fn checkpoint(run: &Run) -> Option<String> {
  run
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().to_string())
}

/// What the agent is told of the proposed `change`.
pub(super) fn told(translate: &Translate, change: &ProposedChange) -> Value {
  let subject = change.subject.clone();
  let (message, proposed) = match &change.change {
    Change::Publish { of, version, .. } => (
      Message::new("orchestrator.tools.publishWaits")
        .with("name", subject.clone())
        .with("version", version.to_string()),
      json!({ "publish": { "kind": of, "name": subject, "version": version } }),
    ),
    Change::Cancel { number, .. } => (
      Message::new("orchestrator.tools.cancelWaits").with("run", number.to_string()),
      json!({ "cancel": { "run": number, "task": subject } }),
    ),
    Change::Merge { number, .. } => (
      Message::new("orchestrator.tools.mergeWaits").with("run", number.to_string()),
      json!({ "merge": { "run": number, "task": subject } }),
    ),
    Change::Answer {
      number,
      request,
      choices,
      text,
      ..
    } => (
      Message::new("orchestrator.tools.answerWaits").with("run", number.to_string()),
      json!({ "answer": { "run": number, "task": subject, "question": request, "choices": choices, "text": text } }),
    ),
    Change::Message { number, text, .. } => (
      Message::new("orchestrator.tools.messageWaits").with("run", number.to_string()),
      json!({ "message": { "run": number, "task": subject, "text": text } }),
    ),
    Change::Continue {
      number, step, choice, ..
    } => (
      Message::new("orchestrator.tools.continueWaits").with("run", number.to_string()),
      json!({ "continue": { "run": number, "task": subject, "step": step, "choice": choice } }),
    ),
  };
  json!({
    "done": false,
    "waits_for_person": true,
    "proposed": proposed,
    "message": translate.text(&message),
  })
}

/// The resource of kind `kind` named by its id or, ignoring case, its name (a profile also by its role), and its
/// name as people read it (a profile's role).
fn draft_named(db: &ProjectDb, kind: Publishing, named: &str) -> Result<(ResourceId, String), ToolError> {
  let resources = db.resources(resource_kind(kind))?;
  let lowered = named.to_lowercase();
  let by_id = resources.iter().find(|resource| resource.id().to_string() == named);
  let found = by_id.or_else(|| {
    resources
      .iter()
      .find(|resource| resource.name().as_str().to_lowercase() == lowered)
  });
  if kind != Publishing::Profile {
    return found
      .map(|resource| (resource.id(), resource.name().as_str().to_owned()))
      .ok_or_else(|| ToolError::not_found(missing(kind), named));
  }
  if let Some(resource) = found {
    return Ok((resource.id(), role_of(db, resource.id())?));
  }
  for resource in &resources {
    let role = role_of(db, resource.id())?;
    if role.to_lowercase() == lowered {
      return Ok((resource.id(), role));
    }
  }
  Err(ToolError::not_found(Missing::Profile, named))
}

/// The role of profile `id`, as its draft or else its latest version holds it.
fn role_of(db: &ProjectDb, id: ResourceId) -> Result<String, StoreError> {
  let draft = db
    .draft::<AgentProfile>(id)?
    .map(|draft| draft.body().role().as_str().to_owned());
  match draft {
    Some(role) => Ok(role),
    None => Ok(
      db.latest_version::<AgentProfile>(id)?
        .map(|version| version.body().role().as_str().to_owned())
        .unwrap_or_default(),
    ),
  }
}

/// When the draft of `resource` was saved and the version it would be published as; `None` without a draft.
fn mark(db: &ProjectDb, kind: Publishing, resource: ResourceId) -> Result<Option<(Timestamp, u32)>, StoreError> {
  match kind {
    Publishing::Pipeline => mark_of::<Pipeline>(db, resource),
    Publishing::Profile => mark_of::<AgentProfile>(db, resource),
    Publishing::Instruction => mark_of::<Instruction>(db, resource),
    Publishing::Skill => mark_of::<Skill>(db, resource),
    Publishing::Tool => mark_of::<Tool>(db, resource),
  }
}

fn mark_of<B: ResourceBody>(db: &ProjectDb, resource: ResourceId) -> Result<Option<(Timestamp, u32)>, StoreError> {
  let Some(draft) = db.draft::<B>(resource)? else {
    return Ok(None);
  };
  let latest = db.latest_version::<B>(resource)?.map(|version| version.number().get());
  Ok(Some((draft.updated_at(), latest.map_or(1, |number| number + 1))))
}

const fn resource_kind(kind: Publishing) -> ResourceKind {
  match kind {
    Publishing::Pipeline => ResourceKind::Pipeline,
    Publishing::Profile => ResourceKind::AgentProfile,
    Publishing::Instruction => ResourceKind::Instruction,
    Publishing::Skill => ResourceKind::Skill,
    Publishing::Tool => ResourceKind::Tool,
  }
}

const fn missing(kind: Publishing) -> Missing {
  match kind {
    Publishing::Pipeline => Missing::Pipeline,
    Publishing::Profile => Missing::Profile,
    Publishing::Instruction => Missing::Instruction,
    Publishing::Skill => Missing::Skill,
    Publishing::Tool => Missing::Tool,
  }
}
