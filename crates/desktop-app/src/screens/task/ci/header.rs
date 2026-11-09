//! What the header says of the pull request's CI once the run ended with one, or while the agent works on a Fix CI
//! (`docs/design/pr-ci-status.md`, "Header"): the status line names the CI's state (*CI running / failed / passed on
//! #48*, *Fixing CI on #48 · 2 of 3*, *Pull request #48 · CI not read*), and the primary action is *Fix CI* while a
//! check failed, the run's own control while the agent fixes (*Pause*), and *Open pull request* otherwise.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  pull_request::ci::fix::{FixEventKind, FixedBy, failure_of},
  run::RunStatus,
};
use orchester_workspace::{pull_requests::ci_fix::FixOffer, tasks::detail::RunView};

use super::{
  Look, ShownCi,
  actions::{CiActions, FixRequest, open_page, pressed_by},
};
use crate::{
  screens::task::{
    TaskContext,
    header::controls::{Control, Offer, Reason},
  },
  services::Services,
  ui::{button::ButtonKind, icon::Icon, message::message_text, pressable::Press, status::Status},
};

/// The run and its pull request's CI, when the header speaks of it: the run ended having opened it, or its agent
/// works on a Fix CI; nothing waits for the person.
fn shown(task: &TaskContext) -> Option<(&RunView, &ShownCi, Look)> {
  let run = task.detail.run.as_ref()?;
  let shown = task.ci.as_ref()?;
  if task.lost() || task.waiting_approval().is_some() {
    return None;
  }
  let look = shown.look(&run.ci_fixes);
  let status = run.run.status();
  let ended = *status == RunStatus::Succeeded;
  let fixing = look == Look::Fixing && *status == RunStatus::Running;
  (ended || fixing).then_some((run, shown, look))
}

/// The Run section's last event in one sentence, with its glyph (`Last event` `TGEq8` in 10–10d).
pub fn event(ctx: &Ctx, task: &TaskContext) -> Option<(Icon, Arc<str>)> {
  let (run, shown, look) = shown(task)?;
  let number = shown.number().to_string();
  let agent = super::words::agent(ctx, &task.detail, task.lost()).to_string();
  Some(match look {
    Look::Running => (
      Icon::LoaderCircle,
      ctx.t_args("ci.runEvent.running", [("number", number)]),
    ),
    Look::Passed => (
      Icon::CircleCheck,
      ctx.t_args("ci.runEvent.passed", [("number", number)]),
    ),
    Look::Failed => {
      let names = shown
        .read()
        .map(|read| super::words::failing(&read.checks).join(&ctx.t("task.listSeparator")))
        .unwrap_or_default();
      let args = [("number", number), ("names", names), ("agent", agent)];
      (Icon::CircleX, ctx.t_args("ci.runEvent.failed", args))
    }
    Look::Fixing if shown.fix_waits(&run.ci_fixes) => (
      Icon::TriangleAlert,
      ctx.t_args("ci.runEvent.fixWaits", [("number", number)]),
    ),
    Look::Fixing => (
      Icon::Wrench,
      ctx.t_args("ci.runEvent.fixing", [("number", number), ("agent", agent)]),
    ),
    Look::NoChecks | Look::Unknown => {
      let why = ctx.t(super::words::why_not_read(&shown.found.ci)).to_string();
      (
        Icon::TriangleAlert,
        ctx.t_args("ci.runEvent.notRead", [("number", number), ("why", why)]),
      )
    }
  })
}

/// The status line's status and words.
pub fn status(ctx: &Ctx, task: &TaskContext) -> Option<(Status, Arc<str>)> {
  let (run, shown, look) = shown(task)?;
  let number = [("number", shown.number().to_string())];
  Some(match look {
    Look::Running => (Status::Running, ctx.t_args("ci.header.running", number)),
    Look::Passed => (Status::Succeeded, ctx.t_args("ci.header.passed", number)),
    Look::Failed => (Status::Failed, ctx.t_args("ci.header.failed", number)),
    Look::Fixing if shown.fix_waits(&run.ci_fixes) => (Status::Waiting, ctx.t_args("ci.header.fixWaits", number)),
    Look::Fixing => (Status::Working, fixing(ctx, run, shown)),
    Look::NoChecks | Look::Unknown => (Status::Waiting, ctx.t_args("ci.header.notRead", number)),
  })
}

/// *Fixing CI on #48*, with the automatic fix's place under its limit when the switch sent it.
fn fixing(ctx: &Ctx, run: &RunView, shown: &ShownCi) -> Arc<str> {
  let number = shown.number().to_string();
  let sent = shown
    .read()
    .and_then(|read| failure_of(&read.checks))
    .and_then(|failure| run.ci_fixes.sent_for(&failure).map(|event| event.kind));
  match sent {
    Some(FixEventKind::Sent {
      by: FixedBy::Automatically { count, limit },
      ..
    }) => ctx.t_args(
      "ci.header.fixingAuto",
      [
        ("number", number),
        ("count", count.to_string()),
        ("limit", limit.get().to_string()),
      ],
    ),
    _ => ctx.t_args("ci.header.fixing", [("number", number)]),
  }
}

/// The primary action the CI calls for: *Fix CI* while a check failed, *Open pull request* while CI runs, passed or
/// was not read; `None` while the agent fixes, when the run's own control (*Pause*) moves it on.
pub fn primary(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let (run, shown, look) = shown(task)?;
  match look {
    Look::Fixing => None,
    Look::Failed => Some(fix(ctx, task, (run, shown))),
    Look::Running | Look::Passed | Look::NoChecks | Look::Unknown => {
      let open = open_page(&Services::of(ctx), shown.found.pull_request.url().into());
      Some(Control::new(
        ctx,
        (
          "task-primary-open-pull-request",
          "ci.openPullRequest",
          Icon::ExternalLink,
        ),
        ButtonKind::Primary,
        Offer::Ready(Press::run(open)),
      ))
    }
  }
}

/// *Fix CI*, or why it cannot send now.
fn fix(ctx: &mut Ctx, task: &TaskContext, (run, shown): (&RunView, &ShownCi)) -> Control {
  let actions = CiActions::of(ctx);
  let offer = match (&shown.offer, shown.read()) {
    _ if actions.fix.state().get().is_pending() => Offer::Pending,
    (FixOffer::Unavailable(reason), _) => {
      let reason = message_text(ctx, reason);
      Offer::Unavailable(Reason {
        short: reason.clone(),
        full: reason,
      })
    }
    (FixOffer::Available, None) => Offer::Disabled,
    (FixOffer::Available, Some(read)) => {
      let request = FixRequest {
        project: task.project.open.clone(),
        task: task.detail.task.id(),
        run: run.id(),
        read: read.clone(),
        lost: task.lost(),
        by: pressed_by(),
      };
      let fix = actions.fix;
      Offer::Ready(Press::run(move || {
        fix.run(FixRequest {
          by: pressed_by(),
          ..request.clone()
        });
      }))
    }
  };
  Control::new(
    ctx,
    ("task-primary-fix-ci", "ci.fix", Icon::Wrench),
    ButtonKind::Primary,
    offer,
  )
}
