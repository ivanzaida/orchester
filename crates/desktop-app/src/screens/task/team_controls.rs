//! A team task's actions on its page (`Team authority · redesign` `BZc66` rows 2–3, `docs/design/team-authority.md`
//! §2): Claim, Take over a stale claim (asked first in `02e` `T68vo9`, then a new run starts from the Admin's
//! pipeline), Release, and Run, which starts through the team server ([`crate::team::start`]). Each goes through the
//! dispatcher; what the server's capabilities allow is what is offered. The frame keeps the action and whether Take
//! over asks ([`TeamControls`]), so the header, the Summary's claim line and the menu share them; a refusal is said
//! once, at the top of the Summary (`Notice · Couldn't take over KON-12` `DpoZG` in `02f`), with the task's claim as
//! read again.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_api::v2::{AuthorityRef, Subject};
use orchester_core::{id::ClaimId, time::Timestamp};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  actions::{Action, ClaimAction},
  background,
  runner::Runners,
  sharing::device,
  team_work::{self, TeamWorkError},
};

mod lifecycle;
mod standing;
mod words;

pub use lifecycle::{blocking_dialog, reopen_menu};
pub use standing::start_standing;
use words::{acted_words, started_words};

use super::{TaskContext, header::controls::Pressed, team::TeamTaskShown};
use crate::{
  project::ShownProject,
  screens::settings::orchester::PersonOnly,
  services::Services,
  team::{
    act::{Acted, Acting},
    invalidation::Stale,
    pages::TeamClaim,
    start::{StartTask, Started, Stop, start},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{self, Dialog},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// What a person asks of a team task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamAsk {
  Claim,
  /// Take the stale claim over, then start a new run.
  TakeOver,
  Release,
  Run,
  /// Mark the task as done (decision D4): the claim's holder's or an Admin's, and it ends the claim.
  Done,
  /// Cancel the task: an Admin's, and it ends the claim.
  Cancel,
  /// Reopen the closed task to Todo, which leaves it free (an Admin's).
  BackToTodo,
  /// Reopen the closed task to Backlog, which leaves it free (an Admin's).
  BackToBacklog,
  /// Set the blocked reason the person typed (the claim's holder's or an Admin's); the claim stays.
  Block,
  /// Clear the blocked reason.
  ClearBlocked,
  /// Dismiss the notice of a start that was settled for good: the page does not tell it again.
  Dismiss,
}

/// What came of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamAnswer {
  Acted(TeamAsk, Acted),
  Started(TeamAsk, Started),
}

impl lurq::app::component::DevtoolsInspectable for TeamAnswer {}

pub type TeamAction = FutureAction<TeamAsk, TeamAnswer, ()>;

/// Take over's *Take over and start fresh*, which only a person presses.
const TAKE_OVER: &str = "task-take-over-confirm";

/// Take over's question and who may answer it.
#[derive(Clone)]
pub struct TakingOver {
  /// The question is open.
  pub open: Signal<bool>,
  /// When the person last asked for a start, which the header's not-ready line says (`not_ready`).
  pub pressed: Signal<Option<Pressed>>,
  /// Taking another member's stale claim over ends their authority, which only a person decides: no MCP tool takes a
  /// claim over either.
  pub person: PersonOnly,
  /// Mark as blocked asks for its reason in a dialog, which is open.
  pub blocking: Signal<bool>,
  /// The reason typed in that dialog.
  pub reason: Signal<String>,
  /// Reopen's two choices are open.
  pub reopening: Signal<bool>,
  /// What Reopen's choices open beside.
  pub reopen_anchor: ElementRef,
}

impl TakingOver {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      pressed: ctx.signal(None),
      person: PersonOnly::new(ctx),
      blocking: ctx.signal(false),
      reason: ctx.signal(String::new()),
      reopening: ctx.signal(false),
      reopen_anchor: ElementRef::default(),
    }
  }
}

/// The frame's team actions, which its header, Summary and menu share.
#[derive(Clone)]
pub struct TeamControls {
  pub action: TeamAction,
  /// Take over's question is open.
  pub taking_over: Signal<bool>,
  /// When the person last asked for a start in this window; none for a start recorded before it.
  pub pressed: Signal<Option<Pressed>>,
  /// Mark as blocked's dialog is open, and the reason it holds.
  pub blocking: Signal<bool>,
  pub reason: Signal<String>,
  /// Reopen's two choices are open.
  pub reopening: Signal<bool>,
  pub reopen_anchor: ElementRef,
  person: PersonOnly,
}

impl TeamControls {
  pub fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<Self>()
  }

  /// `ask` once no other is under way.
  pub fn run(&self, ask: TeamAsk) {
    if !self.action.state().get_untracked().is_pending() {
      if matches!(ask, TeamAsk::Run | TeamAsk::TakeOver) {
        self.pressed.set(Some(Pressed(Timestamp::now())));
      }
      self.action.run(ask);
    }
  }

  /// The ask under way.
  pub fn pending(&self) -> bool {
    self.action.state().get().is_pending()
  }

  /// The answer to the last ask, unless it tells nothing more: a claim action the server took (Claim, Take over's
  /// claim, Release, Dismiss) says nothing to tell, and leaves the page to tell what the project's records say of the
  /// start next, as in a window that asked for nothing.
  fn told(&self) -> Option<TeamAnswer> {
    self
      .action
      .state()
      .get()
      .data
      .filter(|answer| !matches!(answer, TeamAnswer::Acted(_, Acted::Done)))
  }
}

/// Provides the frame's [`TeamControls`] for `shown`, the team task as last read in `project`, with Take over's
/// question as `taking_over` holds it; made on every render, so an ask is carried out on the task as the page shows it.
pub fn provide(ctx: &mut Ctx, (project, shown): (&ShownProject, Option<Arc<TeamTaskShown>>), taking_over: &TakingOver) {
  let services = Services::of(ctx);
  let name = project.open.project.name().as_str().to_owned();
  let means = (Acting::of(&services), services.runners, services.programs);
  let reason = taking_over.reason.clone();
  let action = ctx.future_action(move |ask: TeamAsk| {
    let (shown, project, reason) = (shown.clone(), name.clone(), reason.clone());
    let (acting, runners, programs) = (means.0.clone(), means.1.clone(), means.2.clone());
    async move {
      let Some(shown) = shown else {
        return Ok(TeamAnswer::Acted(ask, Acted::Failed));
      };
      let reason = reason.get_untracked();
      Ok(carry_out(ask, (&shown, reason), project, (acting, runners, programs)).await)
    }
  });
  ctx.provide(TeamControls {
    action,
    taking_over: taking_over.open.clone(),
    pressed: taking_over.pressed.clone(),
    blocking: taking_over.blocking.clone(),
    reason: taking_over.reason.clone(),
    reopening: taking_over.reopening.clone(),
    reopen_anchor: taking_over.reopen_anchor.clone(),
    person: taking_over.person.clone(),
  });
}

async fn carry_out(
  ask: TeamAsk,
  (shown, reason): (&TeamTaskShown, String),
  project: String,
  (acting, runners, programs): (Acting, Runners, Programs),
) -> TeamAnswer {
  let source = &shown.source;
  let subject = Subject::Task {
    id: shown.task.task.id(),
  };
  let claim = shown.task.claim.as_ref();
  match ask {
    TeamAsk::Claim => {
      let action = Action::Claim(ClaimAction::Acquire { subject });
      TeamAnswer::Acted(ask, acting.act(source, action).await.0)
    }
    TeamAsk::Release => {
      let Some(claim) = claim else {
        return TeamAnswer::Acted(ask, Acted::Failed);
      };
      let authority = AuthorityRef {
        subject,
        claim_id: claim.id,
        work_id: None,
      };
      let action = Action::Claim(ClaimAction::Release { authority });
      TeamAnswer::Acted(ask, acting.act(source, action).await.0)
    }
    TeamAsk::Dismiss => TeamAnswer::Acted(ask, dismissed(&acting, shown).await),
    TeamAsk::Done
    | TeamAsk::Cancel
    | TeamAsk::BackToTodo
    | TeamAsk::BackToBacklog
    | TeamAsk::Block
    | TeamAsk::ClearBlocked => TeamAnswer::Acted(ask, lifecycle::carry_out(ask, (shown, reason), &acting).await),
    TeamAsk::TakeOver => {
      let Some(observed) = claim.map(|claim| claim.id) else {
        return TeamAnswer::Acted(ask, Acted::Failed);
      };
      let action = Action::Claim(ClaimAction::TakeOver { subject, observed });
      match acting.act(source, action).await {
        (Acted::Done, Some(taken)) => {
          let request = start_task(shown, Some(taken), project);
          TeamAnswer::Started(ask, start(&acting, source, (runners, programs), request).await)
        }
        (acted, _) => TeamAnswer::Acted(ask, acted),
      }
    }
    TeamAsk::Run => {
      let held = held_here(shown, claim).await;
      let request = start_task(shown, held, project);
      TeamAnswer::Started(ask, start(&acting, source, (runners, programs), request).await)
    }
  }
}

/// The claim this computer holds on the task and can start under: the page's claim, if this computer's own records
/// still say it holds it. The page's read of the task is refreshed asynchronously after a start let the claim go, so a
/// Run pressed at once would otherwise name a claim already released; a claim whose records cannot be read is not
/// reused, and the start takes a fresh one.
async fn held_here(shown: &TeamTaskShown, claim: Option<&TeamClaim>) -> Option<ClaimId> {
  let claim = claim.filter(|claim| claim.here && !claim.under_module)?;
  let root = shown.source.root().to_owned();
  match background::run(move || team_work::team_here(&root)).await {
    Ok(Ok(here)) => here.holds(claim.id).then_some(claim.id),
    Ok(Err(error)) => {
      tracing::warn!(%error, "this computer's team claims cannot be read for a start");
      None
    }
    Err(_) => None,
  }
}

/// Marks the start the page shows, settled for good, as seen, and reads the task's start again.
async fn dismissed(acting: &Acting, shown: &TeamTaskShown) -> Acted {
  let Some(operation) = shown.last_start.as_ref().map(|recorded| recorded.operation) else {
    return Acted::Failed;
  };
  let (root, task) = (shown.source.root().to_owned(), shown.task.task.id());
  match background::run(move || team_work::start_seen(&root, task, operation)).await {
    Ok(Ok(())) => {
      Stale::EVERY.apply(&acting.team.client());
      Acted::Done
    }
    // A later start replaced the one the page showed: nothing is marked, and the page reads the newest start, which it
    // tells in its own right.
    Ok(Err(TeamWorkError::StartSuperseded(_))) => {
      Stale::EVERY.apply(&acting.team.client());
      Acted::Done
    }
    Ok(Err(error)) => {
      tracing::warn!(%error, "the notice of a team start cannot be dismissed");
      Acted::Failed
    }
    Err(_) => Acted::Failed,
  }
}

/// The start of `shown`'s work under `claim`, or a claim taken for it.
fn start_task(shown: &TeamTaskShown, claim: Option<ClaimId>, project: String) -> StartTask {
  let module = shown
    .module
    .as_ref()
    .map(|module| (module.module.id(), module.revision, module.branch.clone()));
  StartTask {
    task: (shown.task.task.id(), shown.task.revision),
    claim,
    module,
    pipeline: shown.task.task.details().pipeline,
    project,
  }
}

/// Whether the task's work can start here now: a team task without a run here that has not ended, which this computer
/// holds, or which nobody holds and it may claim. A claim another computer holds, the person's own on their other
/// computer too, starts nothing here (02 `nMUwg`, 02b `XbNXU`).
pub fn can_run(task: &TaskContext) -> bool {
  let Some(team) = &task.team else {
    return false;
  };
  let runs_here = task
    .detail
    .run
    .as_ref()
    .is_some_and(|run| !run.run.status().is_terminal());
  let startable = match &team.task.claim {
    Some(claim) => claim.here && !claim.stale,
    None => team.task.abilities.acquire,
  };
  !runs_here && startable
}

/// The claim another computer holds on the task while this computer has no run of it: someone else's, the person's
/// own on their other computer, or its module's; nothing starts here under it.
pub fn held_elsewhere(task: &TaskContext) -> Option<&crate::team::pages::TeamClaim> {
  if task.detail.run.is_some() {
    return None;
  }
  task.team.as_ref()?.task.claim.as_ref().filter(|claim| !claim.here)
}

/// Take over's question (`Dialog` `he6ir` in `02e`), while it is open and the claim as read again can still be taken
/// over; once it cannot (renewed, released, or the server no longer allows it), the question closes.
pub fn take_over_dialog(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  if !controls.taking_over.get() {
    return None;
  }
  let team = task.team.as_ref().filter(|team| team.task.abilities.take_over);
  let claim = team
    .and_then(|team| team.task.claim.as_ref())
    .filter(|claim| claim.stale && !claim.holder.you && !claim.under_module);
  let (Some(team), Some(claim)) = (team, claim) else {
    controls.taking_over.set(false);
    return None;
  };
  let key = task.key.to_string();
  let holder = claim.holder.name.as_str();
  let computer = device::computer_name();
  let pipeline = pipeline_version(ctx, task);
  let mut lines = vec![
    dialog::line(
      "task-take-over-claim",
      Icon::UserRoundCheck,
      &ctx.t_args(
        "summary.team.takeOver.claim",
        [("computer", computer.as_str()), ("holder", holder)],
      ),
    ),
    dialog::line(
      "task-take-over-fresh",
      Icon::RotateCcw,
      &ctx.t_args(
        "summary.team.takeOver.fresh",
        [("pipeline", pipeline.as_str()), ("holder", holder)],
      ),
    ),
  ];
  if let Some(module) = &team.module {
    let words = ctx.t_args("summary.team.takeOver.merges", [("branch", module.branch.as_str())]);
    lines.push(dialog::line("task-take-over-merges", Icon::GitBranch, &words));
  }
  let (open, ask) = (controls.taking_over.clone(), controls.clone());
  let confirm = ActionButton::new(TAKE_OVER, ButtonKind::Primary, ctx.t("summary.team.claim.takeOver"))
    .icon(Icon::HandHelping)
    .on_click(move || {
      if ask.person.allows(TAKE_OVER) {
        open.set(false);
        ask.run(TeamAsk::TakeOver);
      }
    });
  let title = ctx.t_args(
    "summary.team.takeOver.title",
    [("task", key.as_str()), ("holder", holder)],
  );
  let dialog = Dialog::new(
    "task-take-over",
    Icon::HandHelping,
    title,
    ctx.t("summary.team.takeOver.cancel"),
  )
  .part(dialog::lines("task-take-over-lines", lines))
  .under(controls.person.reason(ctx, TAKE_OVER))
  .action(confirm)
  .render(ctx, &controls.taking_over);
  Some(dialog)
}

/// The task's pipeline by its name on this computer, or the id the server names it by when it is none of the team's
/// pipelines here.
pub fn pipeline_name(task: &TaskContext) -> String {
  published(task)
    .map(|(name, _)| name)
    .or_else(|| task.detail.missing_pipeline.clone())
    .unwrap_or_default()
}

/// The task's pipeline by its name and the version a run of it starts from on this computer (*Feature delivery v3*),
/// or as [`pipeline_name`] names it without one.
pub fn pipeline_version(ctx: &Ctx, task: &TaskContext) -> String {
  match published(task) {
    Some((name, version)) => ctx
      .t_args(
        "summary.properties.pipelineValue",
        [("pipeline", name), ("version", version)],
      )
      .to_string(),
    None => pipeline_name(task),
  }
}

/// The name and the latest published version of the task's pipeline on this computer.
fn published(task: &TaskContext) -> Option<(String, String)> {
  let pipeline = task.detail.task.details().pipeline;
  match task.project.feed.tasks.get() {
    crate::project::TasksState::Ready(tasks) => tasks
      .pipelines
      .iter()
      .find(|published| Some(published.id) == pipeline)
      .map(|published| (published.name.to_string(), published.version.to_string())),
    _ => None,
  }
}

/// What the last ask came to, when it did not do what was asked (`DpoZG` in `02f`), with Try again; or what became of
/// the start that was left waiting ([`start_standing`]): the information notice `xnJyn` while it waits, which retries
/// by itself.
pub fn refusal(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  if controls.pending() {
    return None;
  }
  let key = task.key.to_string();
  let (ask, body, started, dismiss) = match controls.told() {
    Some(TeamAnswer::Acted(ask, acted)) => (ask, acted_words(ctx, task, ask, &acted)?, false, false),
    _ => {
      let standing = start_standing(ctx, task)?;
      if standing.started == Started::Waiting {
        return Some(starting(ctx, &key));
      }
      let body = started_words(ctx, task, standing.ask, &standing.started)?;
      // Once the claim was the person's, what did not happen is the run's start.
      let stopped = matches!(&standing.started, Started::Stopped(stop) if !matches!(stop, Stop::Claim(_)));
      (standing.ask, body, stopped, standing.dismiss)
    }
  };
  let title = match ask {
    _ if started => "summary.team.refused.runTitle",
    TeamAsk::Claim => "summary.team.refused.claimTitle",
    TeamAsk::TakeOver => "summary.team.refused.takeOverTitle",
    TeamAsk::Release => "summary.team.refused.releaseTitle",
    TeamAsk::Run => "summary.team.refused.runTitle",
    TeamAsk::Dismiss => "summary.team.refused.dismissTitle",
    TeamAsk::Done => "summary.team.refused.doneTitle",
    TeamAsk::Cancel => "summary.team.refused.cancelTitle",
    TeamAsk::BackToTodo | TeamAsk::BackToBacklog => "summary.team.refused.reopenTitle",
    TeamAsk::Block => "summary.team.refused.blockTitle",
    TeamAsk::ClearBlocked => "summary.team.refused.clearBlockedTitle",
  };
  let open = controls.taking_over.clone();
  let again = controls.clone();
  let retry = ActionButton::new(
    "task-team-refused-again",
    ButtonKind::Secondary,
    ctx.t("common.tryAgain"),
  )
  .icon(Icon::RefreshCw)
  .on_click(move || match ask {
    TeamAsk::TakeOver => open.set(true),
    ask => again.run(ask),
  });
  let mut notice = Notice::new(
    "task-team-refused",
    NoticeTone::Danger,
    ctx.t_args(title, [("task", key.as_str())]),
  )
  .body(body)
  .action(retry);
  if dismiss {
    let dismissing = controls;
    notice = notice.action(
      ActionButton::new(
        "task-team-refused-dismiss",
        ButtonKind::Secondary,
        ctx.t("summary.team.refused.dismiss"),
      )
      .on_click(move || dismissing.run(TeamAsk::Dismiss)),
    );
  }
  Some(notice.into())
}

/// The notice of a start that waits for the team server or the runner (`Notice/Info` `xnJyn`): not a failure, and with
/// nothing to press, since the project asks again by itself.
fn starting(ctx: &Ctx, key: &str) -> Element {
  Notice::new(
    "task-team-starting",
    NoticeTone::Info,
    ctx.t_args("summary.team.refused.startingTitle", [("task", key)]),
  )
  .body(ctx.t("summary.team.refused.waiting"))
  .into()
}
