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
  core::Signal,
  node::Element,
};
use orchester_api::v2::{AuthorityErrorCode, AuthorityRef, Subject};
use orchester_core::id::ClaimId;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  actions::{Action, ClaimAction},
  runner::Runners,
  sharing::device,
};

use super::{TaskContext, team::TeamTaskShown};
use crate::{
  project::ShownProject,
  screens::settings::orchester::moment,
  services::Services,
  team::{
    act::{Acted, Acting},
    start::{StartTask, Started, Stop, start},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{self, Dialog},
    icon::Icon,
    message::message_text,
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
}

/// What came of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamAnswer {
  Acted(TeamAsk, Acted),
  Started(TeamAsk, Started),
}

impl lurq::app::component::DevtoolsInspectable for TeamAnswer {}

pub type TeamAction = FutureAction<TeamAsk, TeamAnswer, ()>;

/// The frame's team actions, which its header, Summary and menu share.
#[derive(Clone)]
pub struct TeamControls {
  pub action: TeamAction,
  /// Take over's question is open.
  pub taking_over: Signal<bool>,
}

impl TeamControls {
  pub fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<Self>()
  }

  /// `ask` once no other is under way.
  pub fn run(&self, ask: TeamAsk) {
    if !self.action.state().get_untracked().is_pending() {
      self.action.run(ask);
    }
  }

  /// The ask under way.
  pub fn pending(&self) -> bool {
    self.action.state().get().is_pending()
  }
}

/// Provides the frame's [`TeamControls`] for `shown`, the team task as last read in `project`, with Take over's
/// question open while `taking_over` holds; made on every render, so an ask is carried out on the task as the page
/// shows it.
pub fn provide(
  ctx: &mut Ctx,
  (project, shown): (&ShownProject, Option<Arc<TeamTaskShown>>),
  taking_over: &Signal<bool>,
) {
  let services = Services::of(ctx);
  let name = project.open.project.name().as_str().to_owned();
  let means = (Acting::of(&services), services.runners, services.programs);
  let action = ctx.future_action(move |ask: TeamAsk| {
    let (shown, project) = (shown.clone(), name.clone());
    let (acting, runners, programs) = (means.0.clone(), means.1.clone(), means.2.clone());
    async move {
      let Some(shown) = shown else {
        return Ok(TeamAnswer::Acted(ask, Acted::Failed));
      };
      Ok(carry_out(ask, &shown, project, (acting, runners, programs)).await)
    }
  });
  ctx.provide(TeamControls {
    action,
    taking_over: taking_over.clone(),
  });
}

async fn carry_out(
  ask: TeamAsk,
  shown: &TeamTaskShown,
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
      let held = claim
        .filter(|claim| claim.here && !claim.under_module)
        .map(|claim| claim.id);
      let request = start_task(shown, held, project);
      TeamAnswer::Started(ask, start(&acting, source, (runners, programs), request).await)
    }
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
/// holds or may claim.
pub fn can_run(task: &TaskContext) -> bool {
  let Some(team) = &task.team else {
    return false;
  };
  let runs_here = task
    .detail
    .run
    .as_ref()
    .is_some_and(|run| !run.run.status().is_terminal());
  let held = team.task.claim.as_ref().is_some_and(|claim| claim.here && !claim.stale);
  !runs_here && (held || team.task.abilities.acquire)
}

/// Take over's question (`Dialog` `he6ir` in `02e`), while it is open.
pub fn take_over_dialog(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  let team = task.team.as_ref()?;
  let claim = team.task.claim.as_ref()?;
  if !controls.taking_over.get() {
    return None;
  }
  let key = task.key.to_string();
  let holder = claim.holder.name.as_str();
  let computer = device::computer_name();
  let pipeline = pipeline_name(task);
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
  let confirm = ActionButton::new(
    "task-take-over-confirm",
    ButtonKind::Primary,
    ctx.t("summary.team.claim.takeOver"),
  )
  .icon(Icon::Hand)
  .on_click(move || {
    open.set(false);
    ask.run(TeamAsk::TakeOver);
  });
  let title = ctx.t_args(
    "summary.team.takeOver.title",
    [("task", key.as_str()), ("holder", holder)],
  );
  let dialog = Dialog::new(
    "task-take-over",
    Icon::Hand,
    title,
    ctx.t("summary.team.takeOver.cancel"),
  )
  .part(dialog::lines("task-take-over-lines", lines))
  .action(confirm)
  .render(ctx, &controls.taking_over);
  Some(dialog)
}

/// The task's pipeline by its name on this computer, or the slug the server names it by.
pub fn pipeline_name(task: &TaskContext) -> String {
  let pipeline = task.detail.task.details().pipeline;
  let named = match task.project.feed.tasks.get() {
    crate::project::TasksState::Ready(tasks) => tasks
      .pipelines
      .iter()
      .find(|published| Some(published.id) == pipeline)
      .map(|published| published.name.to_string()),
    _ => None,
  };
  named
    .or_else(|| task.detail.missing_pipeline.clone())
    .unwrap_or_default()
}

/// What the last ask came to, when it did not do what was asked (`DpoZG` in `02f`), with Try again.
pub fn refusal(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  let state = controls.action.state().get();
  if state.is_pending() {
    return None;
  }
  let answer = state.data?;
  let key = task.key.to_string();
  let (ask, body) = match &answer {
    TeamAnswer::Acted(ask, acted) => (*ask, acted_words(ctx, task, *ask, acted)?),
    TeamAnswer::Started(ask, started) => (*ask, started_words(ctx, task, *ask, started)?),
  };
  // Once the claim was the person's, what did not happen is the run's start.
  let started = matches!(&answer, TeamAnswer::Started(_, Started::Stopped(stop)) if !matches!(stop, Stop::Claim(_)));
  let title = match ask {
    _ if started => "summary.team.refused.runTitle",
    TeamAsk::Claim => "summary.team.refused.claimTitle",
    TeamAsk::TakeOver => "summary.team.refused.takeOverTitle",
    TeamAsk::Release => "summary.team.refused.releaseTitle",
    TeamAsk::Run => "summary.team.refused.runTitle",
  };
  let open = controls.taking_over.clone();
  let again = controls;
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
  let notice = Notice::new(
    "task-team-refused",
    NoticeTone::Danger,
    ctx.t_args(title, [("task", key.as_str())]),
  )
  .body(body)
  .action(retry);
  Some(notice.into())
}

/// Why a claim command did not do what was asked; `None` when it did.
fn acted_words(ctx: &Ctx, task: &TaskContext, ask: TeamAsk, acted: &Acted) -> Option<Arc<str>> {
  let key = match acted {
    Acted::Done => return None,
    // Someone renewed or took the claim first: the page shows the claim as read again, and says who.
    Acted::Refused(AuthorityErrorCode::ClaimConflict) if ask == TeamAsk::TakeOver => {
      let claim = task.team.as_ref()?.task.claim.as_ref()?;
      let time = moment(ctx, claim.renewed);
      return Some(ctx.t_args(
        "summary.team.refused.renewed",
        [("holder", claim.holder.name.as_str()), ("time", time.as_str())],
      ));
    }
    Acted::Refused(code) => return Some(refused_words(ctx, *code)),
    Acted::Changed(_) => "summary.team.refused.changed",
    Acted::Unavailable => "summary.team.refused.unavailable",
    Acted::SignedOut => "summary.team.refused.signedOut",
    Acted::Failed => "summary.team.refused.failed",
  };
  Some(ctx.t(key))
}

/// Why a start did not start the run; `None` when it did.
fn started_words(ctx: &Ctx, task: &TaskContext, ask: TeamAsk, started: &Started) -> Option<Arc<str>> {
  let stop = match started {
    Started::Run(_) => return None,
    Started::Waiting => return Some(ctx.t("summary.team.refused.waiting")),
    Started::Stopped(stop) => stop,
  };
  Some(match stop {
    Stop::Claim(acted) => return acted_words(ctx, task, ask, acted),
    Stop::NotReady => ctx.t("summary.team.refused.notReady"),
    // A start refused as invalid is the server's own reason, not an agent at work.
    Stop::Refused(AuthorityErrorCode::Invalid) => ctx.t("summary.team.refused.other"),
    Stop::Refused(code) => refused_words(ctx, *code),
    Stop::BranchNotFound(branch) => ctx.t_args("summary.team.refused.branch", [("branch", branch.as_str())]),
    Stop::BindingsRequired(profiles) => ctx.t_args(
      "summary.team.refused.bindings",
      [("profiles", profiles.join(&ctx.t("task.listSeparator")))],
    ),
    Stop::NotReadable => ctx.t("summary.team.refused.notReadable"),
    Stop::Runner(messages) => messages.first().map_or_else(
      || ctx.t("summary.team.refused.failed"),
      |message| message_text(ctx, message),
    ),
    Stop::Failed => ctx.t("summary.team.refused.failed"),
  })
}

/// A refusal's code as the page says it.
pub fn refused_words(ctx: &Ctx, code: AuthorityErrorCode) -> Arc<str> {
  let key = match code {
    AuthorityErrorCode::ClaimConflict => "summary.team.refused.conflict",
    AuthorityErrorCode::ModuleClaimConflict => "summary.team.refused.moduleConflict",
    AuthorityErrorCode::ClaimLost | AuthorityErrorCode::SessionMismatch => "summary.team.refused.lost",
    AuthorityErrorCode::Invalid => "summary.team.refused.busy",
    AuthorityErrorCode::TransitionDenied => "summary.team.refused.denied",
    AuthorityErrorCode::DecisionRequired => "summary.team.refused.decision",
    AuthorityErrorCode::Forbidden => "summary.team.refused.forbidden",
    AuthorityErrorCode::MembershipRevoked => "summary.team.refused.removed",
    AuthorityErrorCode::ModuleRequired => "summary.team.refused.notReady",
    AuthorityErrorCode::Unavailable | AuthorityErrorCode::RateLimited | AuthorityErrorCode::Unauthenticated => {
      "summary.team.refused.unavailable"
    }
    _ => "summary.team.refused.other",
  };
  ctx.t(key)
}
