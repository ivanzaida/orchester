//! Merging a run on a thread of its own, as a person or automation asked.

use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  path::Path,
  sync::Arc,
  thread,
};

use orchester_core::{
  id::RunId,
  integration::{
    MergeError, PreflightStage,
    attempt::{AttemptStage, MergeAsker, MergeVia},
  },
  revision::Revision,
};
use orchester_exec::programs::Programs;

use super::{Active, JobState, RunJobs};
use crate::{
  activity::{Activity, run_operations},
  messages::{self, Message},
  secrets::Vault,
  tasks::{
    integration::{IntegrationProblem, integration_refusal, stage_message},
    local_integration::LegacyPolicy,
    merge_attempts::{self, Asked},
    merge_retry::merge_retrying,
    worktree_removal::WorktreeRemover,
  },
  workspace::OpenProject,
};

impl RunJobs {
  /// Starts merging the run on a thread of its own, as a person asked in the window; whether it began, which it
  /// does not while another operation of the run works or once its merge succeeded.
  pub fn start_merge(&self, args: MergeArgs) -> bool {
    self.start_merge_with(args, (Telling::Person, None))
  }

  /// [`Self::start_merge`] for an MCP client's input to the window: the merge and what it tells go through
  /// `merge_run`'s policy ([`LegacyPolicy`]), made on the merge's thread from `vault`.
  pub fn start_synthetic_merge(&self, args: MergeArgs, vault: Vault) -> bool {
    self.start_merge_with(args, (Telling::Client(vault), None))
  }

  pub fn start_legacy_merge(&self, args: MergeArgs, policy: LegacyPolicy) -> bool {
    self.start_merge_with(args, (Telling::Policy(Box::new(policy)), None))
  }

  /// [`Self::start_legacy_merge`], only while the run's branch is at `expected`, the commit a person reviewed.
  pub fn start_legacy_merge_at(&self, args: MergeArgs, policy: LegacyPolicy, expected: Revision) -> bool {
    self.start_merge_with(args, (Telling::Policy(Box::new(policy)), Some(expected)))
  }

  fn start_merge_with(&self, args: MergeArgs, (telling, expected): (Telling, Option<Revision>)) -> bool {
    if !self.begin(Active::Merge) {
      return false;
    }
    self.update(|state| {
      state.merge = JobState::Running;
      state.conflicts.clear();
      state.merged = None;
      state.continues_locally = false;
    });
    let what = run_operations::run_merge(&args.open, args.task, (&args.key, args.number), &args.branch);
    self.name(&args.open, &what, true);
    let job = self.clone();
    if let Err(error) = thread::Builder::new()
      .name("run-merge".into())
      .spawn(move || job.merge_now(args, (telling, expected.as_ref())))
    {
      tracing::error!(%error, "cannot start run merge thread");
      self.update(|state| state.merge = JobState::Failed(messages::interrupted()));
      self.finish();
    }
    true
  }

  fn merge_now(&self, args: MergeArgs, (telling, expected): (Telling, Option<&Revision>)) {
    let MergeArgs {
      open,
      task,
      key,
      number,
      branch,
      run,
      programs,
      worktrees,
      activity,
      separator,
      changed,
    } = args;
    let running = activity.start(run_operations::run_merge(&open, task, (&key, number), &branch));
    let not_done = Message::new("statusBar.operations.runMerge.notDone")
      .with("task", key.clone())
      .with("run", number.to_string());
    let automated = !matches!(telling, Telling::Person);
    let asked = telling.asked();
    let made_policy = telling.policy(&open.repo_root, run);
    let result = catch_unwind(AssertUnwindSafe(|| {
      let allowed = made_policy.as_ref().map_err(Clone::clone).and_then(|policy| {
        policy
          .as_ref()
          .map_or(Ok(()), |policy| preflight(&open.repo_root, policy))
      });
      if let Err(problem) = allowed {
        merge_attempts::record_refused(&open.repo_root, run, asked, (AttemptStage::Policy, &problem));
        return Err(problem);
      }
      merge_retrying(&worktrees, &programs, (run, expected, None), asked)
    }));
    let policy = made_policy.ok().flatten();
    let continues_locally = matches!(&result, Ok(Err(problem)) if problem.continues_locally());
    let (mut conflicts, mut made) = (Vec::new(), None);
    let merged = match result {
      Ok(Ok(merged)) => {
        made = Some(merged.finish(running, &activity, (&open, task, (&key, number))));
        JobState::Succeeded
      }
      Ok(Err(problem)) => {
        let raw = problem.message(&separator);
        let message = match (&policy, &problem) {
          (Some(policy), _) => policy.safe_message(raw),
          // An automation's policy that could not be made passes no cause through: only the stage is told.
          (None, IntegrationProblem::Preflight { stage, .. }) if automated => stage_message(*stage),
          (None, _) => raw,
        };
        match &problem {
          IntegrationProblem::Merge(MergeError::Conflict { files }) => conflicts.clone_from(files),
          IntegrationProblem::NotRecorded { merge, .. } => made = Some(merge.clone()),
          _ => {}
        }
        if automated {
          running.fail(message.clone());
        } else {
          problem.end(running, not_done);
        }
        JobState::Failed(message)
      }
      Err(_) => {
        tracing::error!("run merge thread panicked");
        running.fail(messages::interrupted());
        JobState::Failed(messages::interrupted())
      }
    };
    if let Some(policy) = &policy {
      conflicts = policy.safe_paths(conflicts);
    }
    self.update(|state| {
      state.merge = merged;
      state.conflicts = conflicts;
      state.merged = made;
      state.continues_locally = continues_locally;
    });
    self.finish();
    changed();
  }
}

/// Who the merge's outcome is told to: the person in full, or an automation through `merge_run`'s policy, given or made
/// from the vault for an MCP client's input to the window.
enum Telling {
  Person,
  Policy(Box<LegacyPolicy>),
  Client(Vault),
}

impl Telling {
  /// Who asked for the merge, and from where: a person or an MCP client pressing *Merge into <base>*, or `merge_run`.
  const fn asked(&self) -> Asked {
    match self {
      Self::Person => (MergeAsker::Person, MergeVia::TaskPage),
      Self::Client(_) => (MergeAsker::Automation, MergeVia::TaskPage),
      Self::Policy(_) => (MergeAsker::Automation, MergeVia::MergeRun),
    }
  }

  /// The policy the merge goes through, none for a person; a policy that cannot be made refuses the merge.
  fn policy(self, root: &Path, run: RunId) -> Result<Option<LegacyPolicy>, IntegrationProblem> {
    match self {
      Self::Person => Ok(None),
      Self::Policy(policy) => Ok(Some(*policy)),
      Self::Client(vault) => LegacyPolicy::new(root, run, vault)
        .map(Some)
        .map_err(integration_refusal(root, PreflightStage::Policy)),
    }
  }
}

/// Refuses a legacy merge whose result git holds already, in full or in part, or whose identities the policy cannot
/// prove.
fn preflight(root: &Path, policy: &LegacyPolicy) -> Result<(), IntegrationProblem> {
  match policy
    .preflight()
    .map_err(integration_refusal(root, PreflightStage::Policy))?
  {
    None => Ok(()),
    Some(_) => Err(IntegrationProblem::Store(Message::new(
      "localIntegration.externalResult",
    ))),
  }
}

#[derive(Clone)]
pub struct MergeArgs {
  pub open: OpenProject,
  pub task: orchester_core::id::TaskId,
  pub key: String,
  pub number: u32,
  pub branch: String,
  pub run: RunId,
  pub programs: Programs,
  /// The project's run worktrees, which its runner removes once the run merged.
  pub worktrees: WorktreeRemover,
  pub activity: Activity,
  pub separator: Arc<str>,
  pub changed: Arc<dyn Fn() + Send + Sync>,
}
