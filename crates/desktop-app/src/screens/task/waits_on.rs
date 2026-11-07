//! A task whose run waits on another task (ORC-784; `docs/design/orchestrator-watch.md` decision 11, 09 `SLuUW`, 10
//! `z5sQf` and `rlnEZ` f in `Orchestrator watch · redesign`): the header's status line says *Waits on KON-52 at 03
//! Implement* after an `hourglass`, 12/500 `text_secondary`, with *Run 17 · since 10:41*, and the header has no primary
//! action; the step the run stands at is neutral. The Summary's last event is the wait's row: on the page's ground in
//! a `border` stroke, 10 by 12, its parts 8 apart, a 13 `hourglass` in `text_muted`, the task and the branch it must be
//! merged into in 12 `text_secondary`, and *Drop the wait* 12/500 `text_primary` at its end. While the wait cannot be
//! met (the task was canceled or closed without the merge) the row is on `warning_muted` in a `warning` stroke with a
//! `warning` glyph, and the status line takes the warning tone. The Conversation ends with the wait's line (`Ejat3`),
//! 10 apart, with *Drop the wait* 12/500 `text_secondary`. Properties show *Depends on: KON-52 · not merged* after a
//! 13 `hourglass` in `text_secondary`.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::Answerer;
use orchester_workspace::{
  display,
  messages::Message,
  orchestrator::{
    waits_on::{
      dropping::drop_wait,
      view::{RunWaitView, Unmet, WaitState},
    },
    watching::LedIn,
  },
  tasks::{detail::TaskDetail, outside::Outside},
  workspace::OpenProject,
};

use super::TaskContext;
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
  },
};

/// The latest run's wait while it holds the run: it waits on the task, or the task ended without its merge and
/// nothing but the person will end it.
pub fn holding(detail: &TaskDetail) -> Option<&RunWaitView> {
  let wait = detail.waits_on.as_ref()?;
  let run = detail.run.as_ref()?;
  (wait.run == run.id() && wait.state.holds() && !run.run.status().is_terminal()).then_some(wait)
}

/// The header's status line while the run waits on a task (`g9ufW` in 09); `None` while it does not.
pub fn status_line(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let wait = holding(&task.detail)?;
  let waited = wait.waited.as_str();
  let unmet = matches!(wait.state, WaitState::Unmet { .. });
  let step = task
    .detail
    .run
    .as_ref()
    .and_then(|run| run.current_step())
    .map(|step| super::step_label(ctx, step.number, &step.name));
  let words = match (unmet, step) {
    (true, _) => ctx.t_args("task.waitsOn.cannotBeMet", [("waited", waited)]),
    (false, Some(step)) => ctx.t_args("task.waitsOn.at", [("waited", waited), ("step", &*step)]),
    (false, None) => ctx.t_args("task.waitsOn.status", [("waited", waited)]),
  };
  let color = if unmet {
    PaletteColor::Warning
  } else {
    PaletteColor::TextSecondary
  };
  let run = ctx.t_args(
    "task.waitsOn.runLine",
    [("run", wait.number.to_string()), ("time", display::clock(wait.since))],
  );
  Some(
    Row::new()
      .id("task-status-row")
      .width(Dimension::Pct(100.0))
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .id("task-status")
          .describe("status", if unmet { "waits_on_unmet" } else { "waits_on" })
          .spacing(ExtraSpacing::Pair.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Hourglass, IconSize::Base, color.clone()).id("task-status-icon"))
          .child(
            Text::new(&words)
              .id("task-status-label")
              .variant(TypographyStyle::Label)
              .color(color)
              .nowrap(),
          ),
      )
      .child(
        Text::new(&run)
          .id("task-run-line")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into(),
  )
}

/// *Depends on* while the run waits on a task: that task, not merged yet.
pub fn depends_on(ctx: &Ctx, task: &TaskContext) -> Option<Element> {
  let wait = holding(&task.detail)?;
  let words = ctx.t_args("summary.waitsOn.notMerged", [("waited", wait.waited.as_str())]);
  Some(
    Row::new()
      .id("task-property-depends")
      .describe("waits_on", wait.waited.clone())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Hourglass, IconSize::Base, PaletteColor::TextSecondary))
      .child(
        Text::new(&words)
          .id("task-property-depends-label")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into(),
  )
}

/// Where the wait's row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  /// The Summary's last event (`Z0g4HO`, `guxGs`).
  Summary,
  /// The Conversation's line after its rows (`Ejat3`).
  Conversation,
}

/// The wait's row on the page of the task `detail` reads, in project `open`, while the wait holds the run; `None` while
/// it does not.
pub fn row(ctx: &mut Ctx, (detail, open): (&TaskDetail, &OpenProject), place: Place) -> Option<Element> {
  let wait = holding(detail)?.clone();
  let leader = match &detail.led_in {
    LedIn::Chat(leader) => Some(leader.title.clone()),
    LedIn::Nobody => None,
  };
  let id = match place {
    Place::Summary => "task-wait",
    Place::Conversation => "conversation-wait",
  };
  Some(ctx.mount_keyed::<WaitRow>(
    id,
    WaitProps {
      id,
      place,
      wait,
      leader,
      open: open.clone(),
    },
  ))
}

/// The row of `wait`, as `place` shows it.
#[derive(Clone, PartialEq)]
pub struct WaitProps {
  id: &'static str,
  place: Place,
  wait: RunWaitView,
  /// The conversation that leads the task, which told the person a wait cannot be met.
  leader: Option<Arc<str>>,
  open: OpenProject,
}

impl DevtoolsInspectable for WaitProps {}

/// The row, with the person's *Drop the wait*: a press made by automation is refused, and says so.
pub struct WaitRow {
  dropping: FutureAction<Answerer, (), Message>,
}

impl Component for WaitRow {
  type Props = WaitProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let props = ctx.props::<WaitProps>().clone();
    let dropping = ctx.future_action(move |by: Answerer| {
      let (services, open, run) = (services.clone(), props.open.clone(), props.wait.run);
      async move {
        drop_wait((&services.runners, &services.servers), &open, run, by).await?;
        services.changes.record(open.project.id(), Outside::Tasks);
        Ok(())
      }
    });
    Self { dropping }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WaitProps>().clone();
    let unmet = matches!(props.wait.state, WaitState::Unmet { .. });
    let words = words(ctx, &props);
    let (glyph, fill, stroke) = match (props.place, unmet) {
      (_, true) => (
        PaletteColor::Warning,
        Some(PaletteColor::WarningMuted),
        PaletteColor::Warning,
      ),
      (Place::Summary, false) => (PaletteColor::TextMuted, None, PaletteColor::Border),
      (Place::Conversation, false) => (PaletteColor::TextSecondary, None, PaletteColor::Border),
    };
    let (words_color, link_color) = match props.place {
      Place::Summary => (PaletteColor::TextSecondary, PaletteColor::TextPrimary),
      Place::Conversation => (PaletteColor::TextSecondary, PaletteColor::TextSecondary),
    };
    let spacing = match props.place {
      Place::Summary => SpacingSize::Sm,
      Place::Conversation => ExtraSpacing::Loose.role(),
    };
    let row = Row::new()
      .id(props.id.to_owned())
      .describe("state", if unmet { "unmet" } else { "waiting" })
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(spacing)
      .align_items(Alignment::Center)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Hourglass, IconSize::Base, glyph))
      .child(
        Text::new(&words)
          .id(format!("{}-text", props.id))
          .variant(TypographyStyle::Description)
          .color(words_color)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(self.drop_link(ctx, &props, link_color));
    let row = match fill {
      Some(fill) => row.background(fill),
      None => row,
    };
    let failed = self.dropping.state().get().error.map(|error| {
      Text::new(&message_text(ctx, &error))
        .id(format!("{}-error", props.id))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::Danger)
        .width(Dimension::Pct(100.0))
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(row)
      .with_children(failed)
  }
}

impl WaitRow {
  /// *Drop the wait*, which takes no press while the drop is under way.
  fn drop_link(&self, ctx: &Ctx, props: &WaitProps, color: PaletteColor) -> Element {
    let pending = self.dropping.state().get().is_pending();
    let label = ctx.t(if pending {
      "summary.waitsOn.dropping"
    } else {
      "summary.waitsOn.drop"
    });
    let dropping = self.dropping.clone();
    pressable(move || {
      if !dropping.state().get_untracked().is_pending() {
        dropping.run(pressed_by());
      }
    })
    .id(format!("{}-drop", props.id))
    .describe("a11y_name", label.clone())
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(Text::new(&label).variant(TypographyStyle::Button).color(color).nowrap())
    .into()
  }
}

/// Who pressed *Drop the wait*: automation when the press came as synthetic input (an MCP client's), else the person.
pub(in crate::screens) fn pressed_by() -> Answerer {
  if is_synthetic_input() {
    tracing::warn!("synthetic input reached Drop the wait; refused as automation's");
    Answerer::Automation
  } else {
    Answerer::Person
  }
}

/// What the row says of the wait, where it shows.
fn words(ctx: &Ctx, props: &WaitProps) -> Arc<str> {
  let wait = &props.wait;
  let waited = wait.waited.as_str();
  let unmet = match wait.state {
    WaitState::Unmet { at, how } => Some((display::clock(at), how)),
    _ => None,
  };
  let Some((time, how)) = unmet else {
    return match props.place {
      Place::Summary if wait.waited_title.is_empty() => ctx.t_args(
        "summary.waitsOn.waitingUntitled",
        [("waited", waited), ("branch", wait.base.as_str())],
      ),
      Place::Summary => ctx.t_args(
        "summary.waitsOn.waiting",
        [
          ("waited", waited),
          ("title", wait.waited_title.as_str()),
          ("branch", wait.base.as_str()),
        ],
      ),
      Place::Conversation => ctx.t_args(
        "conversation.waitsOn.waiting",
        [("waited", waited), ("branch", wait.base.as_str())],
      ),
    };
  };
  let key = match (how, &props.leader) {
    (Unmet::Canceled, Some(_)) => "summary.waitsOn.canceledTold",
    (Unmet::Canceled, None) => "summary.waitsOn.canceled",
    (Unmet::ClosedUnmerged, Some(_)) => "summary.waitsOn.closedTold",
    (Unmet::ClosedUnmerged, None) => "summary.waitsOn.closed",
  };
  let chat = props.leader.as_deref().unwrap_or_default();
  ctx.t_args(key, [("waited", waited), ("time", time.as_str()), ("chat", chat)])
}
