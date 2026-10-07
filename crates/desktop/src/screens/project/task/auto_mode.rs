//! A run's *Auto mode* switch (`orchester_core::run::review`), in its Approvals panel and its permissions
//! popover (no frame yet). In Approvals it follows *Approve everything in this run*: an option with its
//! explanation. In the popover it is a setting of its own, labelled like the others, with a plain switch:
//! it is the run's, not the profile's, so it saves at once and is not part of Apply, which it says. It
//! applies from the next request. Both say which connection and model review. It is fixed while the run
//! has ended, while a change is saved, and while the project has no reviewer, which it says after the lock
//! glyph.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::RunId;

use crate::{
  background,
  messages::{self, Message},
  screens::project::{
    ProjectNav,
    run_approval_action::{RunApprovalChanges, reload_after, run_approval_action},
  },
  tasks::run_approvals::{AutoModeState, ReviewerShown, RunApprovalChange, auto_mode_in, unavailable_key},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    field::wrapping_field_error,
    message_text,
    switch::{Switch, locked_reason},
  },
};

/// Where the switch shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) enum AutoModePlace {
  /// The run's Approvals panel: an option with its explanation.
  Approvals,
  /// The run permissions popover: a labelled setting apart from Apply.
  Popover,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) struct AutoModeProps {
  pub repo_root: PathBuf,
  pub run: RunId,
  /// The run has not ended: its auto mode can still change.
  pub live: bool,
  /// The switch's id, for tooling: one per place it shows.
  pub id: &'static str,
  pub place: AutoModePlace,
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
    let column = Column::new().width(Dimension::Pct(100.0)).overflow_visible();
    let column = match props.place {
      AutoModePlace::Approvals => column.spacing(ExtraSpacing::Stack.role()),
      AutoModePlace::Popover => column
        .id(format!("{}-section", props.id))
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t("runs.autoMode.switch")).variant(TypographyStyle::Caption))
        // Said first: the section is no part of Apply.
        .child(hint(&ctx.t("runs.autoMode.savesAtOnce")).id(format!("{}-saves-at-once", props.id))),
    };
    let Some(AutoModeState { on, reviewer }) = read.data else {
      return column.with_children(read.error.map(|error| wrapping_field_error(&message_text(ctx, &error))));
    };
    let unavailable = reviewer.as_ref().err().map(|why| ctx.t(unavailable_key(*why)));
    let reviewed = reviewer.as_ref().ok().map(|shown| reviewed_by(ctx, shown));
    let reason_id = format!("{}-unavailable", props.id);
    let switch = match props.place {
      AutoModePlace::Approvals => {
        let explanation = ctx.t("runs.autoMode.explanation");
        let note = match &reviewed {
          Some(reviewed) => format!("{explanation} {reviewed}").into(),
          None => explanation,
        };
        let switch = Switch::new(props.id, ctx.t("runs.autoMode.switch"), on)
          .note(note)
          .inset_ring();
        match unavailable.clone() {
          Some(reason) => switch.reason(reason_id.clone(), reason),
          None => switch,
        }
      }
      AutoModePlace::Popover => Switch::new(props.id, ctx.t("runs.autoMode.popoverSwitch"), on),
    };
    let element = if props.live && reviewer.is_ok() && !changing.is_pending() {
      let request = RunApprovalChanges {
        repo_root: props.repo_root.clone(),
        changes: vec![(props.run, RunApprovalChange::AutoMode(!on))],
      };
      switch.render(move || change.run(request.clone()))
    } else {
      switch.locked()
    };
    let mut column = column.child(element);
    if props.place == AutoModePlace::Popover {
      column = column
        .with_children(unavailable.map(|reason| locked_reason(reason_id, &reason)))
        .child(hint(&ctx.t("runs.autoMode.popoverExplanation")))
        .with_children(reviewed.map(|reviewed| hint(&reviewed).id(format!("{}-reviewer", props.id))));
    }
    column.with_children(
      changing
        .error
        .map(|error| wrapping_field_error(&message_text(ctx, &error))),
    )
  }
}

/// *Reviewed by claude-haiku-4-5 on Claude · Kontur* (with the effort, when one is chosen).
pub(in crate::screens::project) fn reviewed_by(ctx: &Ctx, shown: &ReviewerShown) -> Arc<str> {
  match &shown.effort {
    Some(effort) => ctx.t_args(
      "runs.autoMode.reviewedByWithEffort",
      [
        ("model", shown.model.as_str()),
        ("effort", effort.as_str()),
        ("connection", &*shown.connection),
      ],
    ),
    None => ctx.t_args(
      "runs.autoMode.reviewedBy",
      [("model", shown.model.as_str()), ("connection", &*shown.connection)],
    ),
  }
}

/// A muted hint under the popover's switch, as its other settings have.
fn hint(text: &str) -> Text {
  Text::new(text)
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
}

/// The run's auto mode and whether the project can review, read again after each change, including
/// those automation made to the run or to the project's auto mode settings.
fn read(
  ctx: &mut Ctx,
  (props, changing): (&AutoModeProps, bool),
) -> lurq::app::ctx::FutureState<AutoModeState, Message> {
  let outside = ctx
    .use_context::<ProjectNav>()
    .map(|nav| nav.feed.outside.get())
    .map(|outside| (outside.runs, outside.settings));
  let deps = (props.repo_root.clone(), props.run, changing, outside);
  let read = ctx.future(deps, |(repo_root, run, ..)| async move {
    background::run(move || auto_mode_in(&repo_root, run))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot read the run's auto mode");
        messages::store_error(&error)
      })
  });
  read.state().get()
}
