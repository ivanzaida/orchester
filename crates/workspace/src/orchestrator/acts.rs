//! How a call of the chat's tool that changes something goes (`docs/design/orchestrator-toolset.md`, ORC-909). While
//! the project's switch *Let the Orchestrator drive* is on, the call is carried out at once, exactly as the same tool
//! is for any client of the app: refused or done by the tool's own rules, with nothing proposed. Publishing a draft,
//! canceling and merging a run, answering an agent, sending its agents a message, deciding how a stopped run goes on
//! and making the project a git repository are kept in the conversation as the change they made ([`super::change`]).
//! While it is off, those become a proposal the person applies ([`super::change`]), a start too unless the project
//! allows Autopilot ([`super::proposal`]), and in a turn nobody asked for what changes how runs go on is only proposed
//! ([`super::guard`]).

use orchester_core::{
  id::RunId,
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{ProposedChange, Publishing},
  },
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{
  auto_merge, blocked,
  change::{
    self, continuation,
    control::{self, QueueCall, RELEASE_QUEUE, RESUME_RUN, ToldTo},
  },
  created::{self, CREATE_TASK},
  guard,
  keep::{self, HOLD_QUEUE, Named},
  proposal,
  repository::INIT_REPOSITORY,
  tools::{ANSWER_QUESTION, CANCEL_RUN, CONTINUE_RUN, ChatScope, MERGE_RUN, RUN_TASK, SEND_MESSAGE, app_tool},
};
use crate::{
  app_mcp::{Services, ToolError, Translate, blocking},
  runner::Runners,
};

/// Whether the person lets the Orchestrator chat drive in `scope`'s project.
pub(super) async fn drives(services: &Services, scope: &ChatScope) -> Result<bool, ToolError> {
  let (workspace, root) = (services.workspace().clone(), scope.root.clone());
  blocking(move || Ok(workspace.orchestrator_drives(&root)?)).await
}

/// Carries out or proposes the change `name` with `arguments`, given the chat's project, and keeps its action in the
/// conversation.
pub(super) async fn act_and_keep(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Value, ToolError> {
  let acted = act((services, translate), scope, name, arguments).await;
  let Acted { mut result, kept } = acted;
  let outcome = match (&result, kept.waits) {
    (Ok(_), Some(waits)) => waits,
    (Ok(_), None) => ActionOutcome::Done,
    (Err(error), _) => keep::refusal(error, translate),
  };
  let named = kept.task.map(Named::Task).or_else(|| keep::named_task(arguments));
  let named = named.or_else(|| result.as_ref().ok().and_then(keep::created_task));
  // A hold that held new starts keeps what it held, for its card.
  let held = (name == HOLD_QUEUE && outcome == ActionOutcome::Done).then(|| keep::arguments_task(&named));
  // A task created outside what the conversation watches comes back to it, unless another one leads it.
  let watch = match (&mut result, name == CREATE_TASK && outcome == ActionOutcome::Done) {
    (Ok(told), true) => created::watch_created(scope, told).await,
    _ => None,
  };
  let action = ChatAction {
    change: kept.change.map(Box::new),
    started: kept.started,
    resumed: kept.resumed,
    auto_mode: kept.auto_mode,
    on_its_own: kept.on_its_own,
    watch,
    ..ChatAction::new(name, None, outcome)
  };
  keep::keep(scope, (named, held), action).await;
  runners.chat_changed(&scope.root);
  result
}

/// What a change did: what the agent is told, and what its action keeps.
struct Acted {
  result: Result<Value, ToolError>,
  kept: Kept,
}

impl Acted {
  fn done(result: Result<Value, ToolError>) -> Self {
    Self {
      result,
      kept: Kept::default(),
    }
  }
}

/// What an action keeps beyond the call: how it waits for the person when it does, the key of the task it names,
/// what it publishes, cancels or merges, the run it started, and whether it was done without asking because auto
/// mode is on, or merged in a turn nobody asked for.
#[derive(Default)]
struct Kept {
  waits: Option<ActionOutcome>,
  task: Option<String>,
  change: Option<ProposedChange>,
  started: Option<RunId>,
  resumed: Option<RunId>,
  auto_mode: bool,
  on_its_own: bool,
}

/// Carries out the change `name` with `arguments` in `scope`, or proposes it to the person.
async fn act(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Acted {
  let drives = match drives(services, scope).await {
    Ok(drives) => drives,
    Err(error) => return Acted::done(Err(error)),
  };
  // While driving, the tools are called as the chat's own: what only a person's switch allowed a client (accepting a
  // step, raising limits, what a publication changes in agents' prompts) the chat's switch allows.
  let driving;
  let services = if drives {
    driving = services.clone().driven_by_chat();
    &driving
  } else {
    services
  };
  let deps = (services, translate);
  let proposes = match guard::only_proposed(scope, name, drives).await {
    Ok(proposes) => proposes || (!drives && guard::needs_person(name)),
    Err(error) => return Acted::done(Err(error)),
  };
  let acted = match name {
    RUN_TASK => start(deps, scope, arguments, drives).await,
    _ if proposes => propose_change(deps, scope, name, arguments).await,
    MERGE_RUN => merge(deps, scope, arguments).await,
    CANCEL_RUN | INIT_REPOSITORY | ANSWER_QUESTION | SEND_MESSAGE => applied(deps, scope, name, arguments).await,
    CONTINUE_RUN => continuing(deps, scope, arguments).await,
    RESUME_RUN => resume(deps, scope, arguments, drives).await,
    _ if Publishing::of_tool(name).is_some() => publish(deps, scope, name, arguments).await,
    // A hold or a release that changed nothing is refused, so its card shows no change that did not happen.
    HOLD_QUEUE | RELEASE_QUEUE => {
      let call = if name == HOLD_QUEUE {
        QueueCall::Hold
      } else {
        QueueCall::Release(ToldTo::Agent)
      };
      let task = arguments.get("task").and_then(Value::as_str);
      let told = match app_tool(deps, name, arguments.clone()).await {
        Ok(told) => control::queue_changed(told, call, (scope, task)).await,
        refused => refused,
      };
      Ok(Acted::done(told))
    }
    _ => Ok(Acted::done(app_tool(deps, name, arguments.clone()).await)),
  };
  acted.unwrap_or_else(|error| Acted::done(Err(error)))
}

/// A cancel, merge, answer, message, continuation or repository the chat carries out at once, exactly as it was
/// checked ([`change::apply`]: a merge merges only the commit checked), and keeps as its own action. The tool's answer
/// is the agent's: a refusal is the tool's.
async fn applied(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let (change, task) = change::subject((scope, services), name, arguments).await?;
  let result = change::apply((services, translate), scope, &change, &[], Answerer::Automation).await;
  Ok(Acted {
    result,
    kept: Kept {
      task,
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// `merge_run` while driving: merged at once. In a turn nobody asked for, only a run the chat's rule lets it merge
/// is ([`auto_merge::check`]: a task's *Don't merge automatically* keeps its runs from being merged on their own),
/// and the action is kept as done on its own. A `merge_failed` names the next step as any merge of the chat's does
/// ([`change::apply`]).
async fn merge(deps: (&Services, &Translate), scope: &ChatScope, arguments: &JsonObject) -> Result<Acted, ToolError> {
  if scope.on_its_own {
    auto_merge::check(scope, arguments).await?;
  }
  let mut acted = applied(deps, scope, MERGE_RUN, arguments).await?;
  acted.kept.on_its_own = scope.on_its_own;
  Ok(acted)
}

/// `continue_run` while driving: allow_retry, return_to and resolve_conflicts are carried out as changes the
/// conversation keeps; every other choice (accept, raise_limits, raise_step_budget, resume) goes to the tool as it is.
async fn continuing(
  deps: (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  if continuation::kept_as_change(arguments) {
    return applied(deps, scope, CONTINUE_RUN, arguments).await;
  }
  Ok(Acted::done(app_tool(deps, CONTINUE_RUN, arguments.clone()).await))
}

/// `run_task`: a start proposed to the person, or, while the chat drives or the project allows Autopilot, the run
/// started; refused for a task marked Blocked.
async fn start(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
  drives: bool,
) -> Result<Acted, ToolError> {
  blocked::marked(scope, arguments.get("key").and_then(Value::as_str)).await?;
  // In a turn nobody asked for, a start is only proposed, unless the chat drives.
  if !drives && (scope.on_its_own || !allows_autopilot(scope).await?) {
    let (told, proposal) = proposal::propose((services, translate), scope, arguments).await?;
    return Ok(Acted {
      result: Ok(told),
      kept: Kept {
        waits: Some(ActionOutcome::Proposed(Box::new(proposal.start))),
        task: Some(proposal.key),
        ..Kept::default()
      },
    });
  }
  let result = app_tool((services, translate), RUN_TASK, arguments.clone()).await;
  let result = match result {
    Err(error) => Err(blocked::named(scope, arguments.get("key").and_then(Value::as_str), error).await),
    told => told,
  };
  let started = result
    .as_ref()
    .ok()
    .and_then(|told| told.pointer("/run/id"))
    .and_then(Value::as_str)
    .and_then(|id| id.parse::<RunId>().ok());
  Ok(Acted {
    result,
    kept: Kept {
      started,
      auto_mode: true,
      ..Kept::default()
    },
  })
}

/// `resume_run`: proposed, done or refused as [`guard::resume_as`] tells; done, it resumes the run or continues an
/// interrupted one from its last checkpoint through the same tool as the task page's controls, and the run is kept.
async fn resume(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
  drives: bool,
) -> Result<Acted, ToolError> {
  let run = match guard::resume_as(scope, arguments, drives).await? {
    guard::Resume::Propose => return propose_change((services, translate), scope, RESUME_RUN, arguments).await,
    guard::Resume::Now(run) => run,
  };
  let result = app_tool((services, translate), RESUME_RUN, arguments.clone()).await;
  let resumed = result.is_ok().then_some(run);
  Ok(Acted {
    result,
    kept: Kept {
      resumed,
      ..Kept::default()
    },
  })
}

/// A cancel, merge, publication, answer, message or continuation the person applies.
async fn propose_change(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let (change, task) = change::subject((scope, services), name, arguments).await?;
  Ok(Acted {
    result: Ok(change::told(translate, &change)),
    kept: Kept {
      waits: Some(ActionOutcome::Waits),
      task,
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// A publication while the chat drives: published by the tool by its own rules. The chat's switch covers what the
/// publication changes in agents' prompts ([`crate::prompt_changes`]); what widens what agents may do is still
/// refused (`person_only`) while *Let automation change agent permissions* is off.
async fn publish(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let (change, _) = change::subject((scope, services), name, arguments).await?;
  // The tool is given the draft that was checked, by its id: a name could name another one by now.
  let (tool, arguments) = change::tool_arguments(scope, &change);
  let result = app_tool((services, translate), tool, arguments).await;
  Ok(Acted {
    result,
    kept: Kept {
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// Whether `scope`'s project allows Autopilot, which only a person turns on: the chat then starts runs itself.
async fn allows_autopilot(scope: &ChatScope) -> Result<bool, ToolError> {
  let root = scope.root.clone();
  blocking(move || Ok(ProjectDb::open(&root)?.allow_autopilot()?.is_on())).await
}
