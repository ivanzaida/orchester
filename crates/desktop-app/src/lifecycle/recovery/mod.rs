//! The runs the previous session left interrupted, on their project's Overview (`Startup after a crash`
//! `JSRBO`, `AgD9q`): no startup screen or dialog. A compact notice says how many runs were interrupted;
//! *Review runs* opens their list in place, where the person continues all of them from their
//! checkpoints, or none. While they continue, the list stays; afterwards it goes, and each run that did
//! not continue is named with why. After *Don't continue* the notice goes and the runs stay interrupted:
//! each task page offers to continue its run. Until the person decides, the notice comes back at every
//! start. After "Wait for agents to save" nothing is shown. Runs that cannot be read are named, never
//! left out silently. What was read and decided is `orchester_workspace::recovery::Recovery`, the session's.

mod list;
mod notices;
mod table;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, run::InterruptCause};
use orchester_workspace::recovery::{Decision, InterruptedRun, Reading, Recovery, RecoveryState};

use self::notices::{not_continued, unread};
use crate::{
  project::ShownProject,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// The element id of the notice.
pub const NOTICE_ID: &str = "recovery-notice";

/// The notice of `project`'s interrupted runs, if it has any to offer.
pub struct RecoveryNotice {
  /// The list is open (*Review runs*), or closed again (*Hide*).
  open: Signal<bool>,
}

impl Component for RecoveryNotice {
  type Props = ShownProject;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // The Overview's sections stand 24 apart (`q9mf0k`); the notice keeps that room under it only while
    // it shows.
    self.shown(ctx).map_or_else(Element::new, |notice| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().bottom(SpacingSize::Xl))
        .child(notice)
        .into()
    })
  }
}

impl RecoveryNotice {
  /// The notice of what the session read and the person decided, if any.
  fn shown(&self, ctx: &mut Ctx) -> Option<Element> {
    let shown = ctx.props::<ShownProject>().clone();
    let services = Services::of(ctx);
    let state = follow(ctx, &services.recovery);
    let project = shown.open.project.id();
    let runs = state.runs_of(project);
    let decision = state.decision(project);
    let notice = match (&state.reading, decision) {
      (_, Decision::Left) | (Reading::Nothing | Reading::Reading, _) => return None,
      (Reading::Failed(reason), _) => {
        let lines = [message_text(ctx, reason)];
        unread(ctx, (&services.recovery, project), &lines)
      }
      (Reading::Read { .. }, Decision::Continued(continuation)) if !continuation.not_continued.is_empty() => {
        not_continued(ctx, (&services.recovery, project), &continuation)
      }
      (Reading::Read { .. }, Decision::Continued(_)) => return None,
      (Reading::Read { .. }, decision) if runs.is_empty() => {
        let lines = notices::unread_lines(ctx, &state.unread_of(project));
        if lines.is_empty() || decision != Decision::Undecided {
          return None;
        }
        unread(ctx, (&services.recovery, project), &lines)
      }
      (Reading::Read { .. }, Decision::Undecided) if !self.open.get() => self.compact(ctx, project, &runs, &services),
      (Reading::Read { .. }, decision) => {
        let continuing = decision == Decision::Continuing;
        self.list(ctx, (&shown, &state), &runs, continuing)
      }
    };
    Some(notice)
  }
  /// *2 runs were interrupted when Orchester closed*, with *Review runs* and *Continue all*.
  fn compact(&self, ctx: &Ctx, project: ProjectId, runs: &[InterruptedRun], services: &Services) -> Element {
    let open = self.open.clone();
    let count = runs.len();
    Notice::new(NOTICE_ID, NoticeTone::Warning, title(ctx, count))
      .body(ctx.t(plural(
        count,
        "lifecycle.recovery.summary.one",
        "lifecycle.recovery.summary.other",
      )))
      .action(
        ActionButton::new(
          "recovery-review",
          ButtonKind::Secondary,
          ctx.t("lifecycle.recovery.reviewRuns"),
        )
        .on_click(move || open.set(true)),
      )
      .action(
        ActionButton::new(
          "recovery-continue-all",
          ButtonKind::Primary,
          ctx.t(plural(
            count,
            "lifecycle.recovery.continueAll.one",
            "lifecycle.recovery.continueAll.other",
          )),
        )
        .icon(Icon::Play)
        .on_click(continue_all(services, project)),
      )
      .into()
  }
}

/// *Continue all*: the session's recovery continues the project's runs through its runner.
fn continue_all(services: &Services, project: ProjectId) -> impl Fn() + Send + Sync + 'static {
  let (recovery, runners, servers) = (
    services.recovery.clone(),
    services.runners.clone(),
    services.servers.clone(),
  );
  move || recovery.continue_all(project, &runners, &servers)
}

fn title(ctx: &Ctx, count: usize) -> Arc<str> {
  ctx.t_args(
    plural(count, "lifecycle.recovery.title.one", "lifecycle.recovery.title.other"),
    [("count", count.to_string())],
  )
}

/// Why the runs were interrupted, as the list says it: the person's Quit now when that stopped all of
/// them, otherwise that Orchester stopped unexpectedly.
fn body_key(runs: &[InterruptedRun]) -> &'static str {
  let quit = runs.iter().all(|run| run.cause == InterruptCause::QuitNow);
  match (quit, runs.len()) {
    (true, 1) => "lifecycle.recovery.body.quitNow.one",
    (true, _) => "lifecycle.recovery.body.quitNow.other",
    (false, 1) => "lifecycle.recovery.body.crashed.one",
    (false, _) => "lifecycle.recovery.body.crashed.other",
  }
}

/// `one` for one, `other` otherwise.
const fn plural(count: usize, one: &'static str, other: &'static str) -> &'static str {
  if count == 1 { one } else { other }
}

/// The session's recovery; reading it in a render renders again when it changes.
fn follow(ctx: &mut Ctx, recovery: &Recovery) -> RecoveryState {
  let state = ctx.stream(
    recovery.clone(),
    |recovery, emitter: StreamEmitter<RecoveryState, ()>| async move {
      let mut updates = recovery.subscribe();
      loop {
        let state = updates.borrow_and_update().clone();
        if !emitter.emit(state) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  state.state().get().data.unwrap_or_else(|| recovery.state())
}
