//! Window status bar (`pmOpt`, states `keJ9Z`): the 28px bar below every screen with the
//! runner, the latest background activity and the project's counts, and the operations
//! popover (`E1msJS`) that opens upward from the activity item.

mod activity_item;
mod counts;
mod hint;
mod item;
mod open;
mod popover;
mod runner_item;
mod timed;

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::Row,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use self::{
  activity_item::{activity_slot, shows_activity},
  counts::counts,
  hint::{Anchors, Hint, HintAnchor},
  open::Opener,
  runner_item::runner_item,
};
use crate::{
  activity::{Activity, ActivityLog, bar},
  runner::RunnerStatus,
  theme::ExtraColor,
};

/// Bar height (`pmOpt`). A popover or hint sits 6px above the bar (`imT2E`), which is 9px
/// above the 22px item it hangs from.
const STATUS_BAR_HEIGHT: f32 = 28.0;
const POPOVER_GAP: f32 = 9.0;

/// The open project, when there is one.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectBar {
  pub repo_root: PathBuf,
  pub runner: RunnerStatus,
  /// Queued tasks; `None` while the tasks are read.
  pub queued: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatusBarProps {
  /// `None` on the screens without a project (first launch, setup, startup failure).
  pub project: Option<ProjectBar>,
}

impl lurq::app::component::DevtoolsInspectable for StatusBarProps {}

pub struct StatusBar {
  activity: Activity,
  popover: Signal<bool>,
  /// Escape closed the popover; the activity item takes the focus back on the next render.
  escaped: Signal<bool>,
  hint: Signal<Option<Hint>>,
  anchors: Anchors,
}

impl Component for StatusBar {
  type Props = StatusBarProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      activity: Activity::of(ctx),
      popover: ctx.signal(false),
      escaped: ctx.signal(false),
      hint: ctx.signal(None),
      anchors: Anchors::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StatusBarProps>().clone();
    let log = self.log(ctx);
    let opener = Opener::new(ctx, &self.activity, props.project.as_ref(), &self.popover);
    let runner = props
      .project
      .as_ref()
      .map(|project| project.runner.state_for(&project.repo_root));
    let hints = hint::Hints::new(&self.hint, &self.anchors);
    let mut left = Row::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center);
    if let Some(project) = &props.project {
      left = left.child(runner_item(ctx, project, &opener, &hints));
    }
    let slot = bar::activity_slot(runner.as_ref(), &log);
    left = left.with_children(activity_slot(
      ctx,
      &slot,
      &log,
      &opener,
      &hints,
      props.project.is_some(),
    ));
    // The popover hangs from the activity item; without it there is nothing to anchor to.
    let popover = shows_activity(&slot).then(|| popover::popover(ctx, &log, &opener, &self.anchors, &self.escaped));
    if !self.popover.get() && self.escaped.get_untracked() {
      self.escaped.set(false);
      ctx.focus(self.anchors.get(HintAnchor::Activity));
    }
    Row::new()
      .id("status-bar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STATUS_BAR_HEIGHT))
      .padding_horizontal(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .justify(Justify::SpaceBetween)
      .background(PaletteColor::SurfaceBase)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(left)
      .child(counts(ctx, props.project.as_ref(), &opener, &hints))
      .with_children(popover)
      // A hint would cover the popover it belongs to.
      .with_children(hints.popup().filter(|_| !self.popover.get()))
  }
}

impl StatusBar {
  fn log(&self, ctx: &mut Ctx) -> ActivityLog {
    let log = ctx.stream(
      self.activity.clone(),
      |activity, emitter: StreamEmitter<ActivityLog, ()>| async move {
        let mut updates = activity.subscribe();
        loop {
          let log = updates.borrow_and_update().clone();
          if !emitter.emit(log) || updates.changed().await.is_err() {
            return;
          }
        }
      },
    );
    log.state().get().data.unwrap_or_else(|| self.activity.snapshot())
  }
}
