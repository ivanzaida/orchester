//! The Deliver step's column (`03 · Deliver step` `gzNi4`, its variants in `03b` `g6Ecy`): its Mode as radio rows
//! (*Approve to merge*, *Auto-merge*, and *Pull request*, visibly *Coming later*), its merge method with what it
//! does (the squash commit's title and body for *Squash*), the profile its Integrate step resolves conflicts with,
//! and whether Check runs again on the merged result, which Auto-merge keeps on (faded, a lock and why). *Remove
//! step* takes the step off the draft. A pipeline without one says what that means and offers *Add Deliver step*
//! (`03c` `y6Bjyv`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::{Ref, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{DeliverMode, DeliverStep, MergeMethod, StepKind};
use orchester_workspace::{
  merge_queue::rechecks,
  pipelines::deliver::{IntegratorChoice, integrating_step, integrator_profiles},
};

use super::state::State;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, LOCKED_SWITCH_OPACITY, PipelinesText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::{Icon, icon},
    pressable::{Focus, Press},
    row_group::{GroupRow, RowGroup, note},
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind, radio_mark},
  },
};

/// The integrator's select (`Select` `Bd3OV`).
const INTEGRATOR_WIDTH: f32 = 180.0;

#[derive(Clone)]
pub struct DeliverProps {
  pub state: State,
  pub deliver: Option<DeliverStep>,
  pub locked: bool,
}

impl PartialEq for DeliverProps {
  fn eq(&self, other: &Self) -> bool {
    self.deliver == other.deliver && self.locked == other.locked
  }
}

impl DevtoolsInspectable for DeliverProps {}

pub struct DeliverColumn {
  integrator: Signal<IntegratorChoice>,
  /// Whether the step is locked, as last shown.
  locked: Ref<bool>,
}

impl Component for DeliverColumn {
  type Props = DeliverProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DeliverProps>().clone();
    let chosen = props.deliver.as_ref().and_then(DeliverStep::integrator);
    let column = Self {
      integrator: ctx.signal(IntegratorChoice(chosen)),
      locked: ctx.create_ref(props.locked),
    };
    let (state, locked) = (props.state, column.locked.clone());
    ctx.watch(&column.integrator, move |choice| {
      if !locked.get() {
        edit(&state, |deliver| deliver.with_integrator(choice.0));
      }
    });
    column
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeliverProps>().clone();
    self.locked.set(props.locked);
    let column = Column::new()
      .id("deliver-column")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg);
    let Some(deliver) = props.deliver.clone() else {
      return column.child(heading(ctx, &props, None)).child(none_body(ctx, &props));
    };
    let number = props.state.get().shown().steps().len() + 1;
    column
      .child(heading(ctx, &props, Some(number)))
      .child(mode_group(ctx, &props, &deliver))
      .child(merge_group(ctx, &props, &deliver))
      .child(self.conflicts_group(ctx, &props, &deliver))
      .child(check_group(ctx, &props, &deliver))
  }
}

/// Changes the draft's Deliver step with `change`, unless it has none.
fn edit(state: &State, change: impl FnOnce(DeliverStep) -> DeliverStep) {
  state.update(|editing| {
    if editing.viewing.is_some() {
      return;
    }
    if let Some(deliver) = editing.edit.deliver().cloned() {
      editing.edit.set_deliver(Some(change(deliver)));
    }
  });
}

/// Its number and name, and *Remove step* while the step can be edited (`Step heading` `MQhNe`).
fn heading(ctx: &Ctx, props: &DeliverProps, number: Option<usize>) -> Row {
  let remove = number.is_some().then(|| {
    let state = props.state.clone();
    let remove = ActionButton::new("deliver-remove", ButtonKind::DangerGhost, ctx.t("delivery.step.remove"))
      .icon(Icon::Trash2)
      .on_click(move || {
        state.update(|editing| {
          if editing.viewing.is_none() {
            editing.edit.set_deliver(None);
          }
        });
        state.get_untracked().views.close();
      });
    if props.locked { remove.disabled() } else { remove }
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(number.map(|n| Text::new(&format!("{n:02}")).variant(PipelinesText::StepNumber.role())))
    .child(
      Text::new(&ctx.t("delivery.step.title"))
        .id("deliver-title")
        .variant(TypographyStyle::Title),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(remove)
}

/// A radio row of the Mode group.
fn mode_row(
  ctx: &Ctx,
  props: &DeliverProps,
  (id, title, body): (&str, &str, &str),
  (mode, chosen): (DeliverMode, bool),
) -> GroupRow {
  let (state, locked) = (props.state.clone(), props.locked);
  let row = GroupRow::new(id, ctx.t(title))
    .leading(radio_mark(chosen))
    .hint(ctx.t(body))
    .chosen(false);
  if locked {
    return row;
  }
  row.press(Press::run(move || edit(&state, |deliver| deliver.with_mode(mode))))
}

fn mode_group(ctx: &Ctx, props: &DeliverProps, deliver: &DeliverStep) -> RowGroup {
  RowGroup::new("deliver-mode", ctx.t("delivery.step.mode"))
    .row(mode_row(
      ctx,
      props,
      (
        "deliver-mode-approve",
        "delivery.step.approve",
        "delivery.step.approveBody",
      ),
      (
        DeliverMode::ApproveToMerge,
        deliver.mode() == DeliverMode::ApproveToMerge,
      ),
    ))
    .row(mode_row(
      ctx,
      props,
      ("deliver-mode-auto", "delivery.step.auto", "delivery.step.autoBody"),
      (DeliverMode::AutoMerge, deliver.mode() == DeliverMode::AutoMerge),
    ))
    .row(
      GroupRow::new("deliver-mode-pull-request", ctx.t("delivery.step.pullRequest"))
        .muted()
        .leading(radio_mark(false).opacity(crate::theme::DISABLED_OPACITY))
        .hint(ctx.t("delivery.step.pullRequestBody"))
        .value(&ctx.t("delivery.step.comingLater")),
    )
}

fn merge_group(ctx: &Ctx, props: &DeliverProps, deliver: &DeliverStep) -> RowGroup {
  let segments = [
    (
      MergeMethod::MergeCommit,
      "deliver-merge-commit",
      "delivery.step.mergeCommit",
    ),
    (MergeMethod::Squash, "deliver-squash", "delivery.step.squash"),
  ]
  .map(|(method, id, label)| {
    let (state, locked) = (props.state.clone(), props.locked);
    Segment::new(
      id,
      SegmentFace::Text(ctx.t(label)),
      deliver.method() == method,
      move || {
        if !locked {
          edit(&state, |deliver| deliver.with_method(method));
        }
      },
    )
  });
  let hint = match deliver.method() {
    MergeMethod::MergeCommit => ctx.t("delivery.step.mergeCommitReadBack"),
    MergeMethod::Squash => ctx.t("delivery.step.squashTitle"),
  };
  let group = RowGroup::new("deliver-merge", ctx.t("delivery.step.merge")).row(
    GroupRow::new("deliver-method", ctx.t("delivery.step.method"))
      .hint(hint)
      .end(segmented("deliver-method-choice", segments, Focus::Keyboard)),
  );
  match deliver.method() {
    MergeMethod::Squash => group.after(note("deliver-squash-body", &ctx.t("delivery.step.squashBody"))),
    MergeMethod::MergeCommit => group,
  }
}

impl DeliverColumn {
  fn conflicts_group(&self, ctx: &mut Ctx, props: &DeliverProps, deliver: &DeliverStep) -> RowGroup {
    let editing = props.state.get();
    let group = RowGroup::new("deliver-conflicts", ctx.t("delivery.step.conflicts"));
    let row = GroupRow::new("deliver-integrator-row", ctx.t("delivery.step.resolveWith"));
    let Ok(pipeline) = editing.shown().validate() else {
      return group.row(row.muted().hint(ctx.t("delivery.step.integratorUnknown")));
    };
    let profiles = integrator_profiles(&pipeline);
    if profiles.is_empty() {
      return group.row(
        row
          .hint(ctx.t("delivery.step.noIntegrator"))
          .value(&ctx.t("delivery.step.badgeConflictsPerson")),
      );
    }
    let name = |profile| {
      editing
        .opened
        .assignables
        .profile(profile)
        .map_or_else(|| ctx.t("delivery.step.unknownProfile"), |choice| choice.name.clone())
    };
    // Without a profile chosen, Integrate works in the step that implements the change: its profile shows.
    let default = integrating_step(&pipeline, &DeliverStep::default(), |_| true).and_then(|step| step.kind.executor());
    let options: Vec<SelectOption<IntegratorChoice>> = std::iter::once(IntegratorChoice(None))
      .filter(|_| deliver.integrator().is_none())
      .chain(profiles.iter().map(|profile| IntegratorChoice(Some(*profile))))
      .map(|choice| {
        let label = match choice.0.or(default) {
          Some(profile) => name(profile),
          None => ctx.t("delivery.step.unknownProfile"),
        };
        SelectOption::new(choice, label.to_string()).disabled(props.locked)
      })
      .collect();
    let select = SelectField::new(
      "deliver-integrator",
      ctx.t("delivery.step.resolveWith"),
      self.integrator.clone(),
      options,
    )
    .width(Dimension::Px(INTEGRATOR_WIDTH))
    .render(ctx);
    group.row(row.hint(ctx.t("delivery.step.integratorReadBack")).end(select))
  }
}

fn check_group(ctx: &Ctx, props: &DeliverProps, deliver: &DeliverStep) -> RowGroup {
  let toggle = Toggle::new(
    "deliver-recheck",
    ToggleKind::Switch,
    deliver.recheck(),
    ctx.t("delivery.step.recheck"),
  )
  .hide_label();
  let switch: Element = if deliver.recheck_locked() {
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .opacity(LOCKED_SWITCH_OPACITY)
      .describe("locked", "true")
      .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
      .child(toggle.settled())
      .into()
  } else if props.locked {
    toggle.into()
  } else {
    let state = props.state.clone();
    toggle
      .on_change(move |on| edit(&state, |deliver| deliver.with_recheck(on)))
      .into()
  };
  let words = if deliver.recheck_locked() {
    ctx.t("delivery.step.recheckLocked")
  } else if !deliver.recheck() {
    ctx.t("delivery.step.recheckOff")
  } else {
    match check_tools(props) {
      Some(tools) => ctx.t_args("delivery.step.recheckReadBack", [("tools", tools)]),
      None => ctx.t("delivery.step.recheckNoTools"),
    }
  };
  RowGroup::new("deliver-check", ctx.t("delivery.step.check")).row(
    GroupRow::new("deliver-recheck-row", ctx.t("delivery.step.recheck"))
      .hint(words)
      .end(switch),
  )
}

/// The names of the tools the pipeline's checking tool steps run, joined: what Check runs again
/// (`orchester_workspace::merge_queue::rechecks`).
fn check_tools(props: &DeliverProps) -> Option<String> {
  let editing = props.state.get();
  let names: Vec<String> = editing
    .shown()
    .steps()
    .iter()
    .filter(|step| rechecks(&step.key))
    .filter_map(|step| match step.kind {
      StepKind::Tool { tool: Some(tool) } => editing
        .opened
        .assignables
        .tool(tool)
        .map(|choice| choice.name.to_string()),
      _ => None,
    })
    .collect();
  (!names.is_empty()).then(|| names.join(", "))
}

/// A pipeline without a Deliver step: what that means, and *Add Deliver step* (`No Deliver step` `mUY21`).
fn none_body(ctx: &Ctx, props: &DeliverProps) -> Element {
  let state = props.state.clone();
  let add = ActionButton::new("deliver-add", ButtonKind::Primary, ctx.t("delivery.step.add"))
    .icon(Icon::Plus)
    .on_click(move || {
      state.update(|editing| {
        if editing.viewing.is_none() && editing.edit.deliver().is_none() {
          editing.edit.set_deliver(Some(DeliverStep::default()));
        }
      });
    });
  Column::new()
    .id("deliver-none")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(Text::new(&ctx.t("delivery.step.noneTitle")).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&ctx.t("delivery.step.noneBody"))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(props.state.get().viewing.is_none().then(|| {
      Row::new()
        .padding(lurq::node::padding::Padding::new().top(SpacingSize::Xs))
        .child(if props.locked { add.disabled() } else { add })
    }))
    .into()
}

/// The words of the Deliver card: its mode and method.
pub(super) fn executor_words(ctx: &Ctx, deliver: &DeliverStep) -> Arc<str> {
  let method = crate::screens::merge_queue::words::method(ctx, deliver.method());
  match deliver.mode() {
    DeliverMode::ApproveToMerge => ctx.t_args("delivery.step.cardApprove", [("method", method)]),
    DeliverMode::AutoMerge => ctx.t_args("delivery.step.cardAuto", [("method", method)]),
  }
}

/// The Deliver step's column on the page.
pub(super) fn column(ctx: &mut Ctx, state: &State, locked: bool) -> Element {
  let editing = state.get();
  let props = DeliverProps {
    state: state.clone(),
    deliver: editing.shown().deliver().cloned(),
    locked: locked || editing.viewing.is_some(),
  };
  ctx.mount_keyed::<DeliverColumn>(&format!("deliver-column-{}", editing.epoch), props)
}
