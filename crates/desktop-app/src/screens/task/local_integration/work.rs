use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
  events::input_source,
};
use orchester_core::{
  blob::ContentHash,
  integration::local::{CheckedResult, LocalConflict, LocalError, LocalPath, Placement, Resolution},
};
use orchester_workspace::{
  background, base_moved,
  tasks::local_integration::{IntegrationError, LocalIntegration, LocalIntent, LocalOrigin, Preview},
};

use super::TaskContext;
use crate::{
  mcp::input_gate::{InputGate, LocalInput},
  services::Services,
};

#[derive(Clone)]
pub enum Action {
  Refused,
  AuthorityRefused,
  BusyRefused,
  Refresh,
  ReviewCommit(Vec<LocalPath>),
  Commit {
    paths: Vec<LocalPath>,
    tree: String,
    message: String,
  },
  Preserve {
    name: String,
    paths: Vec<LocalPath>,
  },
  Sync,
  Merge {
    exact: Box<CheckedResult>,
    generation: u64,
  },
  Save {
    file: LocalConflict,
    expected: Option<ContentHash>,
    resolution: Resolution,
    candidate: orchester_core::integration::local::EditorCandidate,
    placement: Placement,
    restore: bool,
  },
  SaveDraft {
    file: LocalConflict,
    expected: Option<ContentHash>,
    candidate: orchester_core::integration::local::EditorCandidate,
    generation: u64,
  },
  Continue,
  Restore,
  FinishRestore,
  Reconcile,
  /// Records the merge Git proves was made without Orchester as the record's own.
  RecordMergedOutside,
  Abort,
  Cleanup {
    /// The restored files the person changed since and saw listed, which cleanup leaves as they are.
    accepted: Vec<LocalPath>,
  },
  Cancel,
}

#[derive(Clone)]
pub struct Request {
  pub task: TaskContext,
  pub action: Action,
  pub stop: Arc<AtomicBool>,
  pub input: Option<LocalInput>,
  pub generation: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
  Changed,
  Aborted,
  DraftSaved(orchester_core::integration::local::EditorCandidate),
  Resolved(orchester_core::integration::local::EditorCandidate),
  CommitReview { tree: String, preview: Preview },
}
impl DevtoolsInspectable for Outcome {}

pub use crate::ui::local_integration_error::{Problem, problem};

pub type Work = FutureAction<Request, Outcome, Problem>;

impl Request {
  pub fn initiate(&mut self, gate: &InputGate) {
    self.input = Some(gate.local_input(input_source()));
  }
  fn authorize(&self) -> Result<LocalOrigin, Problem> {
    let input = self
      .input
      .as_ref()
      .ok_or(Problem("local_integration.error.authority"))?;
    if !input.recheck() {
      return Err(Problem("local_integration.error.authority"));
    }
    let origin = input.origin();
    origin
      .authorize(self.action.intent())
      .map_err(|_| Problem("local_integration.error.approval"))?;
    Ok(origin)
  }
}
impl Action {
  fn intent(&self) -> LocalIntent {
    match self {
      Self::Refresh | Self::ReviewCommit(_) | Self::SaveDraft { .. } => LocalIntent::Candidate,
      _ => LocalIntent::ReviewedDecision,
    }
  }
}

pub fn action(ctx: &mut Ctx) -> Work {
  let services = Services::of(ctx);
  ctx.future_action(move |request: Request| {
    let services = services.clone();
    async move {
      if matches!(request.action, Action::BusyRefused) {
        return Err(Problem("local_integration.error.foreign"));
      }
      if matches!(request.action, Action::AuthorityRefused) {
        return Err(Problem("local_integration.error.authority"));
      }
      if matches!(request.action, Action::Refused) {
        return Err(Problem("local_integration.error.approval"));
      }
      let task = request.task.clone();
      let run = task
        .detail
        .run
        .as_ref()
        .ok_or(Problem("local_integration.error.run"))?
        .id();
      let jobs = services.jobs.of(task.project.open.project.id()).of(run);
      let open = task.project.open.clone();
      let feed = task.project.feed.clone();
      let (vault, programs) = (services.vault.clone(), services.programs.clone());
      let result = background::run(move || {
        let _working = jobs
          .local_work(&open, &task.key)
          .ok_or(Problem("local_integration.error.foreign"))?;
        let origin = request.authorize()?;
        let input = request
          .input
          .clone()
          .ok_or(Problem("local_integration.error.authority"))?;
        let intent = request.action.intent();
        let authorize = Arc::new(move || {
          if !input.recheck() {
            return Err(LocalError::AuthorityChanged);
          }
          input.origin().authorize(intent)
        });
        let mut session =
          LocalIntegration::open_authorized_at(&open.repo_root, run, request.generation, authorize).map_err(problem)?;
        session
          .authorize(origin, request.action.intent(), request.generation)
          .map_err(problem)?;
        request.authorize()?;
        if origin == LocalOrigin::WindowAutomation
          && let Action::SaveDraft { candidate, .. } = &request.action
          && session.candidate_sensitive(candidate, vault.get()).map_err(problem)?
        {
          return Err(Problem("local_integration.error.metadata"));
        }
        let outcome = execute(&mut session, request.action, vault.get()).map_err(problem)?;
        if request.stop.load(Ordering::Acquire) {
          session.pause().map_err(problem)?;
        }
        // The run's merge moved its base branch: the runs still at work on it are told (ORC-54).
        if session.operation().merged().is_some() {
          base_moved::after_merge(&open.repo_root, &programs);
        }
        Ok(outcome)
      })
      .await
      .map_err(|_| Problem("local_integration.error.unknown"))?;
      feed.saved();
      result
    }
  })
}

fn execute(
  session: &mut LocalIntegration,
  action: Action,
  vault: &dyn orchester_core::secret::SecretVault,
) -> Result<Outcome, IntegrationError> {
  match action {
    Action::Refresh => session.refresh()?,
    Action::Refused => return Err(LocalError::ApprovalRequired.into()),
    Action::AuthorityRefused => return Err(LocalError::AuthorityChanged.into()),
    Action::BusyRefused => return Err(LocalError::ForeignOperation.into()),
    Action::ReviewCommit(paths) => {
      session.review_current()?;
      let tree = session.selected_commit_tree(&paths)?;
      let preview = session.selected_commit_preview(&tree, vault)?;
      return Ok(Outcome::CommitReview { tree, preview });
    }
    Action::Commit { paths, tree, message } => session.commit(&paths, &tree, &message)?,
    Action::Preserve { name, paths } => session.preserve_selected(&name, &paths)?,
    Action::Sync => {
      session.prepare_if_clean()?;
      session.sync()?;
    }
    Action::Merge { exact, generation } => session.approve_and_merge_at(&exact, generation)?,
    Action::Save {
      file,
      expected,
      resolution,
      candidate,
      placement,
      restore,
    } => {
      if restore {
        session.save_restore_resolution(&file, expected.as_ref(), &resolution, placement)?;
      } else {
        session.save_task_resolution(&file, expected.as_ref(), &resolution)?;
      }
      return Ok(Outcome::Resolved(candidate));
    }
    Action::SaveDraft {
      file,
      expected,
      candidate,
      generation,
    } => {
      session.save_editor_draft(&file, expected.as_ref(), &candidate, generation)?;
      return Ok(Outcome::DraftSaved(candidate));
    }
    Action::Continue => session.continue_task()?,
    Action::Restore => session.restore()?,
    Action::FinishRestore => session.finish_restore()?,
    Action::Reconcile => session.reconcile()?,
    Action::RecordMergedOutside => session.record_merged_outside()?,
    Action::Abort => {
      session.abort_task()?;
      return Ok(Outcome::Aborted);
    }
    Action::Cleanup { accepted } => session.remove_preservation_accepting(&accepted)?,
    Action::Cancel => session.cancel_preservation()?,
  }
  Ok(Outcome::Changed)
}
