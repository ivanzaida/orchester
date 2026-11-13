//! The action that moves the run's merge on (`Merge` `j9nGlG` in 01i, `Bnjrk` in 01j), the same on the top card and
//! as the header's primary action, with no key cap: *Merge* while the run waits to be merged; once a
//! merge did not land, the attempt page's primary action (`Cjprb`): *Resolve* for
//! conflicts, which opens the run's attempt page, where they are resolved; *Record merge* for a merge Git made and
//! Orchester did not record; *Put files back* for files a stopped bring-in left changed; *Try again* else, which puts
//! a run the merge queue stopped back in its place there and merges any other on the job the card's Merge uses.
//!
//! Merging runs on the project's shared operations, the job Orchester's `merge_run` uses. A person's press tells them
//! the outcome in full; an MCP client's press through the window goes through `merge_run`'s policy
//! (`RunJobs::start_synthetic_merge`), which refuses and redacts what the tool would. It is under way while git
//! merges; waits, saying why, while the run's worktree is removed, its pull request is created, Local integration
//! works in the checkout, another run of the project merges, a decision of Local integration waits or a merge waits in
//! the run's worktree for the person to resolve it; and is unavailable with why when the run did not record the
//! branch it started from.
//!
//! *Merge and deliver* ([`deliver`]) stands beside *Merge* for a team task's run that started on its module's branch:
//! the same merge, then the module's delivery to its base asked for in the same press
//! ([`crate::team::deliver_next`]).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{id::RunId, integration::base::RunBaseSource, store::MergeStore};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  detail::RunView,
  integration_jobs::{JobState, MergeArgs, ResolveStep},
};

use super::Merge;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::task::{
    PageState, TaskContext,
    header::controls::{Control, Offer, Reason},
    merge::attempt::{actions::Busy, standing::Standing},
  },
  services::Services,
  team::{act::Acting, base_merge, deliver_next::Next},
  ui::{button::ButtonKind, icon::Icon, pressable::Press},
};

/// The action's element id on the top card; the header's is its own.
const ID: &str = "task-merge";
/// *Merge and deliver* on the top card.
const DELIVER_ID: &str = "task-merge-deliver";

/// What a merging action looks like, and the delivery it asks for once its merge landed.
struct Merging {
  look: (&'static str, &'static str, Icon),
  kind: ButtonKind,
  next: Option<Next>,
}

impl Merging {
  const fn primary(label: &'static str, glyph: Icon) -> Self {
    Self {
      look: (ID, label, glyph),
      kind: ButtonKind::Primary,
      next: None,
    }
  }
}

/// The action, and why it waits for other work while it does.
#[derive(Clone)]
pub struct MergeAction {
  pub control: Control,
  pub waits: Option<Arc<str>>,
}

impl MergeAction {
  fn new(control: Control) -> Self {
    Self { control, waits: None }
  }
}

/// The attempt page's work a merge that did not land asks for, which the task page does too.
#[derive(Clone, Copy)]
enum Work {
  Record,
  PutBack,
  Requeue,
}

/// The action of `task`'s latest run as `merge` stands. While a merge waits in the run's worktree for
/// the person to finish it, as the frame read it, the action that merges waits: the same for the top card, the header
/// and ⌘↵.
pub fn action(ctx: &mut Ctx, task: &TaskContext, merge: Merge) -> Option<MergeAction> {
  let run = task.detail.run.as_ref()?;
  let page = task.attempt.as_deref();
  let in_worktree = task.merge_in_worktree();
  let action = match merge {
    Merge::NotMerged(Standing::Conflicts) if !in_worktree && !resolved(task, run) => resolve(ctx, task, run),
    Merge::NotMerged(Standing::NotRecorded) if !in_worktree => work(ctx, task, run, Work::Record),
    Merge::NotMerged(Standing::LeftChanged) if page.is_some_and(|page| !page.left_changed.is_empty()) => {
      work(ctx, task, run, Work::PutBack)
    }
    Merge::NotMerged(_) if page.is_some_and(|page| page.queue_stopped) && !in_worktree => {
      work(ctx, task, run, Work::Requeue)
    }
    Merge::NotMerged(_) => {
      let how = Merging::primary("task.merge.tryAgain", Icon::RotateCcw);
      merging(ctx, task, run, how, in_worktree)
    }
    Merge::Ready => {
      let how = Merging::primary("task.merge.label", Icon::GitMerge);
      merging(ctx, task, run, how, in_worktree)
    }
  };
  Some(action)
}

/// *Merge and deliver* while a team task's run that started on its module's branch waits to be merged, and the team's
/// read of that module is there: the merge *Merge* makes, then the module's delivery to its base. `None` for any
/// other run: a local run has no module to deliver, and a merge that did not land is tried again on its own first.
pub fn deliver(ctx: &mut Ctx, task: &TaskContext, merge: Merge) -> Option<MergeAction> {
  if merge != Merge::Ready {
    return None;
  }
  let run = task.detail.run.as_ref()?;
  let team = task.team.as_ref()?;
  let module = team.module.as_ref()?;
  let RunBaseSource::Module { module: started_in } = run.run.base_source() else {
    return None;
  };
  if started_in != module.module.id() {
    return None;
  }
  let services = Services::of(ctx);
  let next = Next {
    acting: Acting::of(&services),
    source: team.source.clone(),
    programs: services.programs,
    activity: services.activity,
    open: task.project.open.clone(),
    task: task.detail.task.id(),
    module: started_in,
    name: Arc::from(module.module.name().as_str()),
    branch: run.run.base_branch()?.as_str().to_owned(),
    // Told at the press, which knows whether a person made it.
    person: false,
  };
  let how = Merging {
    look: (DELIVER_ID, "task.merge.deliver", Icon::GitPullRequest),
    kind: ButtonKind::Secondary,
    next: Some(next),
  };
  Some(merging(ctx, task, run, how, task.merge_in_worktree()))
}

/// The conflicts of the last attempt were resolved by hand in the run's worktree, which merges now.
fn resolved(task: &TaskContext, run: &RunView) -> bool {
  let resolve = task.jobs.run(run.id()).resolve;
  resolve.step == Some(ResolveStep::Continue) && resolve.state == JobState::Succeeded
}

/// *Resolve*: the run's attempt page, where its conflicts are resolved.
fn resolve(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> MergeAction {
  let route = Route::Project(
    task.project.open.project.id(),
    ProjectPage::TaskMerge(task.key.clone(), run.run.number()),
  );
  let open = Navigation::of(ctx).to(route);
  MergeAction::new(Control::new(
    ctx,
    (ID, "task.merge.resolve", Icon::CodeXml),
    ButtonKind::Primary,
    Offer::Ready(Press::Open(open)),
  ))
}

/// *Record merge*, *Put files back* or *Try again* in the merge queue, the work of the run's attempt page: under way
/// while it is, waiting while other work of the page or of the run's git is.
fn work(ctx: &mut Ctx, task: &TaskContext, run: &RunView, work: Work) -> MergeAction {
  let (look, mine) = match work {
    Work::Record => ((ID, "task.merge.recordMerge", Icon::Database), Busy::Recording),
    Work::PutBack => ((ID, "task.merge.putFilesBack", Icon::Undo2), Busy::PuttingBack),
    Work::Requeue => ((ID, "task.merge.tryAgain", Icon::RotateCcw), Busy::Requeueing),
  };
  let state = PageState::of(ctx).merge;
  let offer = match state.busy.get() {
    Busy::Idle if task.jobs.run(run.id()).busy() => Offer::Disabled,
    Busy::Idle => {
      let actions = state.actions(ctx, task, run.id());
      Offer::Ready(Press::run(move || match work {
        Work::Record => actions.record_merge(),
        Work::PutBack => actions.put_files_back(),
        Work::Requeue => actions.requeue(),
      }))
    }
    busy if busy == mine => Offer::Pending,
    _ => Offer::Disabled,
  };
  MergeAction::new(Control::new(ctx, look, ButtonKind::Primary, offer))
}

/// *Merge*, *Try again* or *Merge and deliver* as `how` looks, merging the run on the project's shared operations.
fn merging(ctx: &mut Ctx, task: &TaskContext, run: &RunView, how: Merging, in_worktree: bool) -> MergeAction {
  let Merging { look, kind, next } = how;
  let Some(branch) = run.run.base_branch() else {
    let reason = Reason {
      short: ctx.t("task.merge.noBranchShort"),
      full: ctx.t("task.merge.noBranch"),
    };
    return MergeAction::new(Control::new(ctx, look, kind, Offer::Unavailable(reason)));
  };
  if let Some(reason) = base_is_an_admins(ctx, task, run) {
    return MergeAction::new(Control::new(ctx, look, kind, Offer::Unavailable(reason)));
  }
  if task.jobs.run(run.id()).merge == JobState::Running {
    // The one merge under way is told by *Merge*; the action beside it waits with it.
    let offer = if next.is_some() {
      Offer::Disabled
    } else {
      Offer::Pending
    };
    return MergeAction::new(Control::new(ctx, look, kind, offer));
  }
  if let Some(key) = waits(task, run, in_worktree) {
    return MergeAction {
      control: Control::new(ctx, look, kind, Offer::Disabled),
      waits: Some(ctx.t(key)),
    };
  }
  let services = Services::of(ctx);
  let (jobs, vault) = (
    services.jobs.of(task.project.open.project.id()).of(run.id()),
    services.vault,
  );
  let args = merge_args(ctx, task, run, branch.as_str());
  // A merge that does not begin (another one took the project meanwhile) leaves the job's state, which the card
  // shows, as it is.
  let start = move || {
    let synthetic = lurq::app::events::is_synthetic_input();
    let mut args = args.clone();
    if let Some(next) = &next {
      let next = Next {
        person: !synthetic,
        ..next.clone()
      };
      args.changed = then_deliver(args.changed.clone(), next, args.run);
    }
    if synthetic {
      jobs.start_synthetic_merge(args, vault.clone());
    } else {
      jobs.start_merge(args);
    }
  };
  MergeAction::new(Control::new(ctx, look, kind, Offer::Ready(Press::run(start))))
}

/// Why this person does not merge `run`: a team task in no module merges into the base itself, which a module's
/// delivery would take an Admin's authorization or the team's Automatic merge for, so a member merges it only while
/// Automatic merge is on ([`base_merge`]). `None` for an Admin, for a run on its module's branch and for a local
/// task's run. While the team's policy is not read, a member's merge waits for it.
fn base_is_an_admins(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Option<Reason> {
  let team = task.team.as_ref()?;
  if matches!(run.run.base_source(), RunBaseSource::Module { .. }) {
    return None;
  }
  if !base_merge::member_waits(ctx, &team.source, &team.standing) {
    return None;
  }
  Some(Reason {
    short: ctx.t("task.merge.adminShort"),
    full: ctx.t("task.merge.admin"),
  })
}

/// What a merge job does when it ends, for *Merge and deliver*: `changed`, then `next`'s delivery when the merge of
/// `run` landed, as the project's database records it. The job calls it on its own thread, where the delivery may
/// wait for the server and for git.
fn then_deliver(changed: Arc<dyn Fn() + Send + Sync>, next: Next, run: RunId) -> Arc<dyn Fn() + Send + Sync> {
  Arc::new(move || {
    changed();
    let merged = ProjectDb::open(&next.open.repo_root).and_then(|db| db.run_merge(run));
    match merged {
      Ok(Some(_)) => next.clone().deliver(),
      // The merge did not land: there is nothing new to deliver, and the card says why.
      Ok(None) => {}
      Err(error) => tracing::warn!(%error, "cannot read whether the run merged; its module's delivery is not asked"),
    }
  })
}

/// Why a merge of `run` waits for other work now, as a key; `None` when it can begin.
fn waits(task: &TaskContext, run: &RunView, in_worktree: bool) -> Option<&'static str> {
  let job = task.jobs.run(run.id());
  if task.jobs.merging().is_some_and(|other| other != run.id()) {
    return Some("task.merge.waits.anotherMerge");
  }
  if job.creating_pull_request() {
    return Some("task.merge.waits.pullRequest");
  }
  if job.removing() {
    return Some("task.merge.waits.removal");
  }
  if job.resolving() || in_worktree {
    return Some("task.merge.waits.resolving");
  }
  if job.busy() {
    return Some("task.merge.waits.localWork");
  }
  // A merge would not begin while a person answers Local integration (`RunJobs::local_decision`).
  task
    .jobs
    .local_decision_pending()
    .then_some("task.merge.waits.decision")
}

/// The merge of `run` into `branch`, reported in the status bar; once it ends the task is read again.
fn merge_args(ctx: &mut Ctx, task: &TaskContext, run: &RunView, branch: &str) -> MergeArgs {
  let services = Services::of(ctx);
  let feed = task.project.feed.clone();
  MergeArgs {
    open: task.project.open.clone(),
    task: task.detail.task.id(),
    key: task.detail.task.key().to_string(),
    number: run.run.number().get(),
    branch: branch.to_owned(),
    run: run.id(),
    worktrees: services.worktree_remover(&task.project.open),
    programs: services.programs,
    activity: services.activity,
    separator: ctx.t("task.listSeparator"),
    changed: Arc::new(move || feed.saved()),
  }
}
