//! The row of a run made to wait on a task (`wait_on_task`, ORC-766) in the Orchestrator's answer, as the wait stands
//! now (ORC-784; `rlnEZ` a–e and 04 `OKtX2` in `Orchestrator watch · redesign`): an event line whose words name the
//! run and the task, with what it waits for in 12 `text_muted` filling the line. While it waits, an `hourglass` in
//! `text_secondary` and *Drop the wait* under the pointer (`lguJb`); continued once the task was merged, a
//! `circle-check` in `success` and *Open ↗* (`lbth2`); the task canceled or closed without the merge, an `hourglass` in
//! `warning` and *Drop the wait* always (`lmGw0`, `NtC6p`); dropped, an `hourglass` in `text_muted` and *Open ↗*
//! (`mJqaY`). The row changes in place: no agent turn is started by a drop.


use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  node::Element,
};
use orchester_core::{id::ProjectId, orchestrator::WaitsOnRow, time::Timestamp};
use orchester_workspace::{
  display,
  messages::Message,
  orchestrator::waits_on::{
    dropping::drop_wait,
    view::{RunWaitView, Unmet, WaitState},
  },
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::line::{Line, LineLink, Tone};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  services::Services,
  ui::{icon::Icon, pressable::Press},
};

#[derive(Clone, PartialEq)]
pub(super) struct WaitsOnProps {
  pub(super) id: String,
  /// The run made to wait, by the action's own row, and the waiting task's key.
  pub(super) row: WaitsOnRow,
  pub(super) task: String,
  /// How the wait stands now; `None` while the project no longer keeps the run.
  pub(super) wait: Option<RunWaitView>,
  /// When the action was kept, said unless the row is `listed` among the answer's lines.
  pub(super) at: Timestamp,
  pub(super) listed: bool,
  pub(super) project: ProjectId,
  pub(super) open: OpenProject,
}

impl DevtoolsInspectable for WaitsOnProps {}

/// The row, with the person's *Drop the wait*.
pub(super) struct WaitsOnLine {
  dropping: FutureAction<(), (), Message>,
}

impl Component for WaitsOnLine {
  type Props = WaitsOnProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let props = ctx.props::<WaitsOnProps>().clone();
    let dropping = ctx.future_action(move |()| {
      let (services, open, run) = (services.clone(), props.open.clone(), props.row.run);
      async move {
        drop_wait((&services.runners, &services.servers), &open, run).await?;
        services.changes.record(open.project.id(), Outside::Tasks);
        Ok(())
      }
    });
    Self { dropping }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WaitsOnProps>().clone();
    let state = props.wait.as_ref().map(|wait| wait.state);
    let line = words(ctx, &props, state);
    let line = match state {
      Some(state) if state.holds() => {
        let pending = self.dropping.state().get().is_pending();
        let dropping = self.dropping.clone();
        let label = ctx.t(if pending {
          "orchestrator.waitsOn.dropping"
        } else {
          "orchestrator.waitsOn.drop"
        });
        let press = Press::run(move || {
          if !dropping.state().get_untracked().is_pending() {
            dropping.run(());
          }
        });
        // A wait nothing else will end offers it always; one that waits, under the pointer (`lguJb`).
        line
          .link_on_hover(state == WaitState::Waiting && !pending)
          .link(Some(LineLink { label, press }))
      }
      _ => line.link(open(ctx, &props)),
    };
    let time = (!props.listed).then(|| display::clock(props.at));
    line.time(time).render(ctx)
  }
}

/// The line's look and words for a wait that stands at `state` (`None`: not read), without its link.
fn words(ctx: &Ctx, props: &WaitsOnProps, state: Option<WaitState>) -> Line {
  let (task, run) = (props.task.as_str(), props.row.number.to_string());
  let waited = props.row.waited.as_str();
  let named = [("task", task), ("run", run.as_str()), ("waited", waited)];
  let branch = props.wait.as_ref().map_or("", |wait| wait.base.as_str());
  let at = |at: Timestamp| display::clock(at);
  let (glyph, tone, title, status, note) = match state {
    None => (Icon::Hourglass, Tone::Plain, "orchestrator.waitsOn.waits", "unread", None),
    Some(WaitState::Waiting) => (
      Icon::Hourglass,
      Tone::Plain,
      "orchestrator.waitsOn.waits",
      "waiting",
      Some(ctx.t_args(
        "orchestrator.waitsOn.whenMerged",
        [("waited", waited), ("branch", branch)],
      )),
    ),
    Some(WaitState::Continued { at: when }) => (
      Icon::CircleCheck,
      Tone::Success,
      "orchestrator.waitsOn.continued",
      "continued",
      Some(ctx.t_args(
        "orchestrator.waitsOn.merged",
        [("waited", waited), ("branch", branch), ("time", &at(when))],
      )),
    ),
    Some(WaitState::Unmet { at: when, how }) => {
      let note = match how {
        Unmet::Canceled => ctx.t_args(
          "orchestrator.waitsOn.canceled",
          [("waited", waited), ("time", &at(when))],
        ),
        Unmet::ClosedUnmerged => ctx.t_args("orchestrator.waitsOn.closed", [("waited", waited)]),
      };
      let status = match how {
        Unmet::Canceled => "canceled",
        Unmet::ClosedUnmerged => "closed",
      };
      (
        Icon::Hourglass,
        Tone::Warning,
        "orchestrator.waitsOn.stillWaits",
        status,
        Some(note),
      )
    }
    Some(WaitState::Dropped { at: when }) => (
      Icon::Hourglass,
      Tone::Quiet,
      "orchestrator.waitsOn.noLonger",
      "dropped",
      Some(ctx.t_args("orchestrator.waitsOn.dropped", [("time", at(when))])),
    ),
    Some(WaitState::Over { at: when }) => (
      Icon::Hourglass,
      Tone::Quiet,
      "orchestrator.waitsOn.noLonger",
      "over",
      Some(ctx.t_args(
        "orchestrator.waitsOn.over",
        [("waited", waited), ("time", &at(when))],
      )),
    ),
  };
  let line = Line::new(props.id.clone(), (glyph, tone), ctx.t_args(title, named))
    .outcome("done")
    .status(status);
  match note {
    Some(note) => line.note(note),
    None => line,
  }
}

/// *Open ↗*, the waiting task's Conversation, where the run goes on.
fn open(ctx: &mut Ctx, props: &WaitsOnProps) -> Option<LineLink> {
  let key = props.task.parse().ok()?;
  let route = Route::Project(
    props.project,
    ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)),
  );
  Some(LineLink {
    label: ctx.t("orchestrator.line.open"),
    press: Press::Open(Navigation::of(ctx).to(route)),
  })
}
