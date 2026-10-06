//! A run's *Auto mode* switch (`orchester_core::run::review`), in its Approvals panel and its permissions
//! popover (no frame yet; the switch and its note follow *Approve everything in this run*). It saves at
//! once, for the whole run, and applies from the next request. It is fixed while the run has ended, while
//! a change is saved, and while the project has no Claude Code connection to review with, which it says.

use std::path::PathBuf;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::RunId;

use crate::{
  background,
  messages::{self, Message},
  screens::project::run_approval_action::{RunApprovalChanges, reload_after, run_approval_action},
  tasks::run_approvals::{AutoModeState, RunApprovalChange, auto_mode_in},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{field::wrapping_field_error, message_text, switch::Switch},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) struct AutoModeProps {
  pub repo_root: PathBuf,
  pub run: RunId,
  /// The run has not ended: its auto mode can still change.
  pub live: bool,
  /// The switch's id, for tooling: one per place it shows.
  pub id: &'static str,
}

impl lurq::app::component::DevtoolsInspectable for AutoModeProps {}

pub(in crate::screens::project::task) struct AutoModeSwitch;

impl Component for AutoModeSwitch {
  type Props = AutoModeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let change = run_approval_action(ctx);
    reload_after(ctx, &change);
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AutoModeProps>().clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let change = run_approval_action(ctx);
    let changing = change.state().get();
    let read = read(ctx, (&props, changing.is_pending()));
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role());
    let Some(AutoModeState { on, reviewer }) = read.data else {
      return column.with_children(read.error.map(|error| wrapping_field_error(&message_text(ctx, &error))));
    };
    let switch = Switch::new(props.id, ctx.t("runs.autoMode.switch"), on)
      .note(ctx.t("runs.autoMode.explanation"))
      .inset_ring();
    let element = if props.live && reviewer && !changing.is_pending() {
      let request = RunApprovalChanges {
        repo_root: props.repo_root.clone(),
        changes: vec![(props.run, RunApprovalChange::AutoMode(!on))],
      };
      switch.render(move || change.run(request.clone()))
    } else {
      switch.locked()
    };
    column = column.child(element);
    if !reviewer {
      column = column.child(
        Text::new(&ctx.t("runs.autoMode.unavailable"))
          .id(format!("{}-unavailable", props.id))
          .variant(ExtraTypography::Hint.role()),
      );
    }
    column.with_children(
      changing
        .error
        .map(|error| wrapping_field_error(&message_text(ctx, &error))),
    )
  }
}

/// The run's auto mode and whether the project can review, read again after each change.
fn read(
  ctx: &mut Ctx,
  (props, changing): (&AutoModeProps, bool),
) -> lurq::app::ctx::FutureState<AutoModeState, Message> {
  let deps = (props.repo_root.clone(), props.run, changing);
  let read = ctx.future(deps, |(repo_root, run, _)| async move {
    background::run(move || auto_mode_in(&repo_root, run))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot read the run's auto mode");
        messages::store_error(&error)
      })
  });
  read.state().get()
}
