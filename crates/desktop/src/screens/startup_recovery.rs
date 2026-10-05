//! The start-up recovery (02.E `o8DCd`) over the first screen: after Quit now or a crash it finds
//! the runs the previous session left interrupted across the registered projects and asks
//! once. *Continue all* continues every one of them through its project's runner, started in
//! the background when that project is not shown; *Don't continue* leaves them interrupted.
//! *View changes* leaves them interrupted too and opens the run's Changes tab, where its task
//! page offers to continue it.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::{Column, Modal, ModalTarget},
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::{ProjectId, RunId},
  time::Timestamp,
};
use orchester_ipc::SessionEnd;

use super::recovery_dialog::{RecoveryActions, RecoveryDialog, RecoveryDialogProps};
use crate::{
  background,
  messages::{self, Message},
  navigation::route::{Page, Route, TaskPageTab},
  programs::Programs,
  recovery::{self, InterruptedRun},
  runner::{RunRequestError, Runners},
  tabs::{OpenIn, Tabs},
  tasks::run::control_message,
  ui::message_text,
  workspace::Workspace,
};

#[derive(Clone, PartialEq)]
pub struct StartupRecoveryProps {
  pub workspace: Workspace,
  pub programs: Programs,
  /// How the previous session ended.
  pub previous: SessionEnd,
  /// When this session started.
  pub session_start: Timestamp,
  /// The project the window shows, once the start screen is known.
  pub shown_project: Option<ProjectId>,
}

impl lurq::app::component::DevtoolsInspectable for StartupRecoveryProps {}

/// The runs the recovery lists, as read.
#[derive(Clone, Debug, PartialEq)]
struct InterruptedRuns(Arc<[InterruptedRun]>);

impl lurq::app::component::DevtoolsInspectable for InterruptedRuns {}

type ContinueAction = FutureAction<Arc<[InterruptedRun]>, (), Message>;

pub struct StartupRecovery {
  /// Answered: the runs continued, or the user chose not to continue them.
  answered: Signal<bool>,
}

impl Component for StartupRecovery {
  type Props = StartupRecoveryProps;

  fn create(ctx: &mut Ctx) -> Self {
    continue_action(ctx);
    Self {
      answered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StartupRecoveryProps>().clone();
    let action = continue_action(ctx);
    let closed = || Modal::new(Column::new()).open_when(false);
    let finished = {
      let state = action.state().get();
      !state.is_pending() && state.data.is_some()
    };
    if self.answered.get() || finished || !recovery::reports(props.previous) {
      return closed();
    }
    let runs = self.runs(ctx, &props);
    if runs.is_empty() {
      return closed();
    }
    let state = action.state().get();
    let continuing = state.is_pending();
    let error = state.error.filter(|_| !continuing).map(|reason| {
      ctx.t_args(
        "lifecycle.recovery.notContinued",
        [("reason", &*message_text(ctx, &reason))],
      )
    });
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting the recovery");
    let (answered, first, resume) = (self.answered.clone(), runs[0].project_root.clone(), runs.clone());
    let props = RecoveryDialogProps {
      previous: props.previous,
      runs,
      shown_project: props.shown_project,
      continuing,
      error,
      actions: RecoveryActions {
        open_log: Arc::new(move || runners.open_log_of(&first)),
        dont_continue: Arc::new(move || answered.set(true)),
        continue_all: Arc::new(move || action.run(resume.clone())),
        view_changes: self.view_changes(ctx),
      },
    };
    Modal::new(ctx.mount_keyed::<RecoveryDialog>("recovery-dialog", props))
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
  }
}

impl StartupRecovery {
  /// Answers the dialog as *Don't continue* would, and opens the run's task page on its Changes
  /// tab as a new history entry of the active tab.
  fn view_changes(&self, ctx: &mut Ctx) -> Arc<dyn Fn(&InterruptedRun) + Send + Sync> {
    let tabs = ctx
      .use_context::<Tabs>()
      .expect("the window provides Tabs before mounting the recovery");
    let answered = self.answered.clone();
    Arc::new(move |run: &InterruptedRun| {
      answered.set(true);
      let page = Page::Task(run.task, TaskPageTab::Changes);
      tabs.open(Route::Project(run.project_id, page), OpenIn::Current);
    })
  }

  /// The interrupted runs, once read; none while they are read or when reading failed (logged:
  /// the runs stay interrupted and each can be continued from its task page).
  fn runs(&self, ctx: &mut Ctx, props: &StartupRecoveryProps) -> Arc<[InterruptedRun]> {
    let deps = (
      props.workspace.clone(),
      props.programs.clone(),
      props.previous,
      props.session_start,
    );
    let load = ctx.future(deps, |(workspace, programs, previous, since)| async move {
      background::run(move || recovery::interrupted_runs(&workspace, &programs, previous, since))
        .await?
        .map(|runs| InterruptedRuns(runs.into()))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the interrupted runs");
          messages::store_error(&error)
        })
    });
    load
      .state()
      .get()
      .data
      .map_or_else(|| Arc::from([]), |InterruptedRuns(runs)| runs)
  }
}

fn continue_action(ctx: &mut Ctx) -> ContinueAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting the recovery");
  let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
  ctx.future_action(move |runs: Arc<[InterruptedRun]>| {
    let (runners, servers) = (runners.clone(), servers.clone());
    async move {
      // A run of a shared task continues only under the person's claim; on any failure the
      // claims this recovery took are released (a run that did continue keeps its claim).
      let mut claims = Vec::new();
      let continued = continue_claimed(&runners, &servers, &runs, &mut claims).await;
      if continued.is_err() {
        for claim in claims {
          claim.release(&servers).await;
        }
      }
      continued
    }
  })
}

/// Takes the claims of `runs` into `claims`, then asks each project's runner to continue them.
async fn continue_claimed(
  runners: &Runners,
  servers: &crate::sharing::Servers,
  runs: &[InterruptedRun],
  claims: &mut Vec<crate::sharing::project::WorkClaim>,
) -> Result<(), Message> {
  for run in runs {
    claims.push(
      servers
        .claim_for_work(&run.project_root, run.task)
        .await
        .map_err(|refusal| refusal.message())?,
    );
  }
  for (root, name, ids) in by_project(runs) {
    let continued = runners
      .continue_runs(&root, &name, ids)
      .await
      .map_err(|error| control_message(&error))?;
    if let Some(refused) = continued.refused.into_iter().next() {
      tracing::warn!(run = %refused.run, rejection = ?refused.rejection, "an interrupted run did not continue");
      return Err(control_message(&RunRequestError::Rejected(refused.rejection)));
    }
  }
  Ok(())
}

/// The runs grouped by project, in the order the projects first appear.
fn by_project(runs: &[InterruptedRun]) -> Vec<(PathBuf, Arc<str>, Vec<RunId>)> {
  let mut groups: Vec<(PathBuf, Arc<str>, Vec<RunId>)> = Vec::new();
  for run in runs {
    match groups.iter_mut().find(|(root, ..)| *root == run.project_root) {
      Some((.., ids)) => ids.push(run.run),
      None => groups.push((run.project_root.clone(), run.project.clone(), vec![run.run])),
    }
  }
  groups
}
