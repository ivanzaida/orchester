//! `get_changes` and `merge_run`: what a run changed, as the task's Changes tab reads it, and its work
//! merged as the task Summary's *Merge into main* merges it, through the same job: one merge or
//! removal at a time per task, the status bar's operation, the merge record and the screens told.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  diff::{Diff, FileChange},
  id::{OperationId, RunId},
  integration::{
    MergeKind, PreflightStage,
    attempt::{AttemptStage, MergeAsker, MergeVia},
  },
  revision::Revision,
  run::{Run, RunStatus},
  store::{MergeStore, RunStore, TaskStore},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use super::super::{
  Translate,
  runs::{InRun, find_run},
  team, wire,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  messages::Message,
  tasks::{
    changes::{ChangesError, Checkpointed, NotCheckpointed, RunRange, RunStage, not_checkpointed_in, run_changes_in},
    delivery::{self, Delivery},
    detail::run_store_error,
    integration::{IntegrationProblem, integration_refusal, stage_message},
    integration_jobs::{JobState, MergeArgs, ProjectJobState},
    local_integration::{CheckoutHolder, IntegrationError, LegacyPolicy, LocalIntent, LocalOrigin, checkout_holder},
    merge_attempts::{self, Asked},
    outside::Outside,
    recorded::task_of,
  },
};

pub async fn get_changes(services: &Services, args: InRun) -> Result<Value, ToolError> {
  let unreadable = |error: ChangesError| ToolError::refused("cannot_read_changes", vec![error.message()]);
  let (open, run, link, checkpointed) = {
    let services = services.clone();
    blocking(move || {
      let open = services.project(&args.project)?;
      let db = ProjectDb::open(&open.repo_root)?;
      let run = find_run(&db, &args.run)?;
      // The changes are read without their task; only the link needs it, so a task that cannot be read costs the
      // link alone.
      let link = match task_of(&db, run.task()) {
        Ok(task) => task.map(|task| Link::Task(open.project.id(), task.key().clone(), TaskPage::Changes).path()),
        Err(_) => None,
      };
      let checkpointed =
        run_changes_in(&open.repo_root, &services.programs, &RunRange::of(&run)).map_err(unreadable)?;
      Ok((open, run, link, checkpointed))
    })
    .await?
  };
  // What the run's worktree holds is read by the project's runner, as the task page's Changes reads it.
  let stage = RunStage::of(run.status());
  let worktree = not_checkpointed_in(&services.runners, &open, (run.id(), stage))
    .await
    .map_err(unreadable)?;
  Ok(json!({
    "run": { "id": run.id().to_string(), "number": run.number().get() },
    "checkpointed": checkpointed_json(&checkpointed),
    "not_checkpointed": not_checkpointed_json(&worktree),
    "link": link,
  }))
}

fn checkpointed_json(changes: &Checkpointed) -> Value {
  match changes {
    Checkpointed::NoCheckpoint => json!({ "state": "no_checkpoint" }),
    Checkpointed::Read { head, changes } => {
      json!({ "state": "read", "from": changes.from.to_string(), "to": head.to_string(), "diff": diff_json(&changes.diff) })
    }
    Checkpointed::TooLarge { head, size } => json!({
      "state": "too_large",
      "to": head.to_string(),
      "files": size.files,
      "additions": size.additions,
      "deletions": size.deletions,
    }),
  }
}

fn not_checkpointed_json(changes: &NotCheckpointed) -> Value {
  match changes {
    NotCheckpointed::Clean => json!({ "state": "clean" }),
    NotCheckpointed::Removed => json!({ "state": "worktree_removed" }),
    NotCheckpointed::Read(changes) => {
      json!({ "state": "read", "from": changes.from.to_string(), "diff": diff_json(&changes.diff) })
    }
    NotCheckpointed::TooLarge { from, size } => json!({
      "state": "too_large",
      "from": from.to_string(),
      "files": size.files,
      "additions": size.additions,
      "deletions": size.deletions,
    }),
  }
}

fn diff_json(diff: &Diff) -> Value {
  let files: Vec<Value> = diff
    .files()
    .iter()
    .map(|file| {
      let change = match file.change() {
        FileChange::Added => "added",
        FileChange::Deleted => "deleted",
        FileChange::Modified => "modified",
        FileChange::Renamed { .. } => "renamed",
        FileChange::Copied { .. } => "copied",
      };
      json!({
        "path": file.path(),
        "old_path": file.old_path(),
        "change": change,
        "binary": file.is_binary(),
        "additions": file.additions(),
        "deletions": file.deletions(),
      })
    })
    .collect();
  json!({ "files": files, "additions": diff.additions(), "deletions": diff.deletions() })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct MergeRun {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// A team project only: the operation id of the Admin's delivery authorization the module's delivery cites. Leave
  /// it out for the team's automatic merge, and for a local project.
  #[serde(default)]
  pub authorization: Option<String>,
}

/// `merge_run`: a local project's run merged into its base branch; a team project's run has its module's delivery
/// requested instead ([`team::merge_run`]).
pub async fn merge_run(services: &Services, args: MergeRun, translate: &Translate) -> Result<Value, ToolError> {
  let authorization = args
    .authorization
    .as_deref()
    .map(|named| {
      named.parse::<OperationId>().map_err(|_| {
        ToolError::InvalidArguments(format!(
          "`authorization` is an operation id such as a UUID, not `{named}`"
        ))
      })
    })
    .transpose()?;
  if let Some(team) = team::of(services, &args.project).await? {
    return team::merge_run(services, &team, (&args.run, authorization), translate).await;
  }
  if authorization.is_some() {
    return Err(ToolError::InvalidArguments(
      "`authorization` cites a team module delivery's authorization; a local project's merge_run takes none".to_owned(),
    ));
  }
  let args = InRun {
    project: args.project,
    run: args.run,
  };
  merge_at(services, (args, None), translate).await
}

/// `merge_run` of run `run` of `project`, only while the run's branch is at `expected`, the commit a person reviewed
/// in a proposal of the Orchestrator chat: a branch that moved since is refused and nothing is merged.
pub(crate) async fn merge_proposed(
  services: &Services,
  translate: &Translate,
  (project, run): (String, String),
  expected: Revision,
) -> Result<Value, ToolError> {
  merge_at(services, (InRun { project, run }, Some(expected)), translate).await
}

async fn merge_at(
  services: &Services,
  (args, expected): (InRun, Option<Revision>),
  translate: &Translate,
) -> Result<Value, ToolError> {
  let separator: Arc<str> = translate.text(&Message::new("common.listSeparator")).into();
  let started = {
    let services = services.clone();
    blocking(move || start(&services, &args, (separator, expected))).await?
  };
  let (mut updates, run, root) = started;
  let ended = updates
    .wait_for(|state| {
      let job = state.run(run);
      !job.busy() && matches!(job.merge, JobState::Succeeded | JobState::Failed(_))
    })
    .await
    .map(|state| state.run(run).merge)
    .map_err(|_| ToolError::interrupted())?;
  if let JobState::Failed(reason) = ended {
    return Err(ToolError::refused("merge_failed", vec![reason]));
  }
  blocking(move || {
    let merge = ProjectDb::open(&root)?
      .run_merge(run)?
      .ok_or_else(|| ToolError::refused("merge_failed", vec![Message::new("appMcp.errors.mergeNotRecorded")]))?;
    let kind = match merge.kind {
      MergeKind::FastForward => "fast_forward",
      MergeKind::MergeCommit => "merge_commit",
      MergeKind::AlreadyMerged => "already_merged",
      MergeKind::Squash => "squash",
    };
    Ok(json!({
      "merge": {
        "run": run.to_string(),
        "into": merge.into.to_string(),
        "revision": merge.revision.to_string(),
        "kind": kind,
        "merged_at": wire::time(merge.merged_at),
      }
    }))
  })
  .await
}

/// The project's merge and removal states to wait on, the run merging, and its repository.
type Started = (watch::Receiver<ProjectJobState>, RunId, PathBuf);

/// Checks that the Summary would offer *Merge into main* for the run, then starts the merge.
fn start(
  services: &Services,
  args: &InRun,
  (separator, expected): (Arc<str>, Option<Revision>),
) -> Result<Started, ToolError> {
  let open = services.project(&args.project)?;
  let db = ProjectDb::open(&open.repo_root)?;
  // A team task's run goes into its module's branch, and only the module's delivery takes it to the base branch.
  if team::linked(&db)? {
    return Err(team::merge_refused());
  }
  let run = find_run(&db, &args.run)?;
  ready(&db, &run, (&open.repo_root, &services.programs))?;
  let jobs = services.jobs.of(open.project.id());
  let holder = checkout_holder(&db, run.id())?;
  if holder == Some(CheckoutHolder::RunningProgram) {
    return Err(refused("program_running", "localIntegration.programRunning"));
  }
  if holder.is_some() || jobs.state().local_decision_pending() {
    LocalOrigin::AuthenticatedDomain
      .authorize(LocalIntent::ReviewedDecision)
      .map_err(|_| refused("needs_decision", "localIntegration.personRequired"))?;
    return Err(refused("needs_decision", "localIntegration.personRequired"));
  }
  let root = open.repo_root.as_path();
  let policy = LegacyPolicy::new(root, run.id(), services.vault.clone())
    .map_err(|error| policy_refused((root, run.id()), None, error))?;
  if let Some(evidence) = policy
    .preflight()
    .map_err(|error| policy_refused((root, run.id()), Some(&policy), error))?
  {
    let problem = IntegrationProblem::Store(Message::new("localIntegration.externalResult"));
    merge_attempts::record_refused(root, run.id(), MERGE_RUN, (AttemptStage::Policy, &problem));
    return Err(refused(
      if evidence.kind == orchester_core::integration::local::ResultEvidenceKind::CompleteAncestry {
        "already_integrated"
      } else {
        "scoped_integration_required"
      },
      "localIntegration.externalResult",
    ));
  }
  let task_runs = db.runs_of_task(run.task()).map_err(run_store_error)?;
  let busy = task_runs.iter().any(|listed| {
    let id = listed.as_ref().map_or_else(|unreadable| unreadable.id, Run::id);
    jobs.of(id).busy()
  });
  if busy {
    return Err(refused("merge_running", "appMcp.errors.integrationRunning"));
  }
  let task = db
    .task(run.task())?
    .ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
  let branch = run
    .base_branch()
    .ok_or_else(|| refused("no_base_branch", "appMcp.errors.noBaseBranch"))?
    .to_string();
  let updates = jobs.subscribe();
  let (changes, project) = (services.changes.clone(), open.project.id());
  let merging = MergeArgs {
    open: open.clone(),
    task: task.id(),
    key: task.key().to_string(),
    number: run.number().get(),
    branch,
    run: run.id(),
    programs: services.programs.clone(),
    worktrees: services.worktree_remover(&open),
    activity: services.activity.clone(),
    separator,
    changed: Arc::new(move || {
      changes.record(project, Outside::Tasks);
    }),
  };
  let job = jobs.of(run.id());
  let started = match expected {
    Some(expected) => job.start_legacy_merge_at(merging, policy, expected),
    None => job.start_legacy_merge(merging, policy),
  };
  // Another merge or removal of the run took it between the check above and now.
  if !started {
    return Err(refused("merge_running", "appMcp.errors.integrationRunning"));
  }
  Ok((updates, run.id(), open.repo_root))
}

/// Refused unless the run is its task's latest, succeeded, and was not merged yet: by Orchester, or outside it as git
/// in the checkout at `root` proves it, as the task's Summary tells it.
fn ready(db: &ProjectDb, run: &Run, (root, programs): (&Path, &Programs)) -> Result<(), ToolError> {
  if !matches!(run.status(), RunStatus::Succeeded) {
    return Err(refused("run_not_ready", "appMcp.errors.runNotReady"));
  }
  let latest = db
    .runs_of_task(run.task())
    .map_err(run_store_error)?
    .iter()
    // A later run that cannot be read is still a later run.
    .filter_map(|listed| match listed {
      Ok(listed) => Some(listed.number().get()),
      Err(unreadable) => unreadable.number.map(|number| number.get()),
    })
    .max();
  if latest != Some(run.number().get()) {
    return Err(refused("not_latest_run", "appMcp.errors.notLatestRun"));
  }
  if db.run_merge(run.id())?.is_some() {
    return Err(refused("already_merged", "appMcp.errors.alreadyMerged"));
  }
  if crate::merge_queue::delivered_by_queue(db, run)? {
    return Err(refused("delivered_by_queue", "appMcp.errors.deliveredByQueue"));
  }
  if let Delivery::MergedOutside(into) = delivery::of_run(programs, root, run) {
    return Err(ToolError::refused(
      "already_merged",
      vec![Message::new("appMcp.errors.mergedOutside").with("branch", into.to_string())],
    ));
  }
  Ok(())
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}

/// `merge_run` of `run` in the project at `root` refused by its policy for `error`, as its attempt is recorded: the
/// stage, then the cause only as `policy` lets an MCP client read it; without a policy, the stage alone.
fn policy_refused((root, run): (&Path, RunId), policy: Option<&LegacyPolicy>, error: IntegrationError) -> ToolError {
  let problem = integration_refusal(root, PreflightStage::Policy)(error);
  merge_attempts::record_refused(root, run, MERGE_RUN, (AttemptStage::Policy, &problem));
  let mut reasons = vec![stage_message(PreflightStage::Policy)];
  if let Some(policy) = policy {
    reasons.push(policy.safe_message(problem.message("")));
  }
  ToolError::refused("integration_preflight_failed", reasons)
}

/// Who asks for `merge_run`'s merges, and from where.
const MERGE_RUN: Asked = (MergeAsker::Automation, MergeVia::MergeRun);
