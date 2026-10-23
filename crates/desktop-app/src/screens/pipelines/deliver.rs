//! The Deliver step's inspector (`Inspector` `rwwek` in `qukzW`, `yxq0Q` in `OqgbW`): its mode (*Approve to merge*,
//! *Auto-merge*, and *Pull request*, visibly not available yet), its merge method with what it does (the squash
//! commit's title and body for *Squash*), the profile its Integrate step resolves conflicts with, and whether Check
//! runs again on the merged result, which Auto-merge keeps on (faded, a lock and why). *Remove step* takes the step
//! off the draft. A pipeline without one shows what that means and *Add Deliver step*.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
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

use super::{parts, state::State};
use crate::{
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize, LOCKED_SWITCH_OPACITY},
  ui::{
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::{Icon, icon},
    inspector::{control, section, section_title},
    option_card::OptionCard,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind},
  },
};

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

pub struct DeliverInspector {
  integrator: Signal<IntegratorChoice>,
  /// Whether the step is locked, as last shown.
  locked: Ref<bool>,
}

impl Component for DeliverInspector {
  type Props = DeliverProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DeliverProps>().clone();
    let chosen = props.deliver.as_ref().and_then(DeliverStep::integrator);
    let inspector = Self {
      integrator: ctx.signal(IntegratorChoice(chosen)),
      locked: ctx.create_ref(props.locked),
    };
    let (state, locked) = (props.state.clone(), inspector.locked.clone());
    ctx.watch(&inspector.integrator, move |choice| {
      if !locked.get() {
        edit(&state, |deliver| deliver.with_integrator(choice.0));
      }
    });
    inspector
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DeliverProps>().clone();
    self.locked.set(props.locked);
    let editing = props.state.get();
    let number = editing.shown().steps().len() + 1;
    let eyebrow = ctx.t_args("pipelines.step", [("number", format!("{number:02}"))]);
    let title = Text::new(&ctx.t("delivery.step.title"))
      .id("deliver-title")
      .variant(ExtraTypography::Strong.role())
      .into();
    let Some(deliver) = props.deliver.clone() else {
      let body = none_body(ctx, &props);
      return parts::panel_with_title(ctx, &eyebrow, title, body, None, Some(props.state));
    };
    // 10 between the sections (`b13kg`).
    let body = parts::panel_body()
      .spacing(ExtraSpacing::Loose.role())
      .child(mode_section(ctx, &props, &deliver))
      .child(merge_section(ctx, &props, &deliver))
      .child(self.conflicts_section(ctx, &props, &deliver))
      .child(check_section(ctx, &props, &deliver));
    let footer = remove_footer(ctx, &props);
    parts::panel_with_title(ctx, &eyebrow, title, body.into(), Some(footer), Some(props.state))
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

fn mode_section(ctx: &Ctx, props: &DeliverProps, deliver: &DeliverStep) -> Column {
  let card = |id: &str, (glyph, title, body): (Icon, &str, &str), mode: DeliverMode| {
    let (state, locked) = (props.state.clone(), props.locked);
    let card = OptionCard::new(
      id.to_owned(),
      (glyph, ctx.t(title)),
      ctx.t(body),
      deliver.mode() == mode,
      move || {
        if !locked {
          edit(&state, |deliver| deliver.with_mode(mode));
        }
      },
    );
    if locked && deliver.mode() != mode {
      card.waiting()
    } else {
      card
    }
  };
  Column::new()
    .id("deliver-mode")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(section_title("deliver-mode-title", &ctx.t("delivery.step.mode"), None))
    .child(card(
      "deliver-mode-approve",
      (Icon::Hand, "delivery.step.approve", "delivery.step.approveBody"),
      DeliverMode::ApproveToMerge,
    ))
    .child(card(
      "deliver-mode-auto",
      (Icon::Zap, "delivery.step.auto", "delivery.step.autoBody"),
      DeliverMode::AutoMerge,
    ))
    .child(
      OptionCard::new(
        "deliver-mode-pull-request",
        (Icon::GitPullRequest, ctx.t("delivery.step.pullRequest")),
        ctx.t("delivery.step.pullRequestBody"),
        false,
        || {},
      )
      .unavailable(ctx.t("delivery.step.comingLater")),
    )
}

fn merge_section(ctx: &Ctx, props: &DeliverProps, deliver: &DeliverStep) -> Column {
  let segments = [
    (MergeMethod::MergeCommit, "deliver-merge-commit", "delivery.step.mergeCommit"),
    (MergeMethod::Squash, "deliver-squash", "delivery.step.squash"),
  ]
  .map(|(method, id, label)| {
    let (state, locked) = (props.state.clone(), props.locked);
    Segment::new(id, SegmentFace::Text(ctx.t(label)), deliver.method() == method, move || {
      if !locked {
        edit(&state, |deliver| deliver.with_method(method));
      }
    })
  });
  let mut section = section("deliver-merge", SpacingSize::Xs)
    .child(section_title("deliver-merge-title", &ctx.t("delivery.step.merge"), None))
    .child(control(
      "deliver-method",
      &ctx.t("delivery.step.method"),
      Row::new().child(segmented("deliver-method-choice", segments, Focus::Keyboard)),
    ));
  section = match deliver.method() {
    MergeMethod::MergeCommit => section.child(read_back("deliver-method-read-back", &ctx.t("delivery.step.mergeCommitReadBack"))),
    MergeMethod::Squash => section.child(control(
      "deliver-squash-message",
      &ctx.t("delivery.step.squashMessage"),
      Column::new()
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t("delivery.step.squashTitle")).variant(ExtraTypography::Note.role()).color(PaletteColor::TextPrimary))
        .child(Text::new(&ctx.t("delivery.step.squashBody")).variant(ExtraTypography::Note.role())),
    )),
  };
  section
}

impl DeliverInspector {
  fn conflicts_section(&self, ctx: &mut Ctx, props: &DeliverProps, deliver: &DeliverStep) -> Column {
    let editing = props.state.get();
    let section = section("deliver-conflicts", SpacingSize::Xs)
      .child(section_title("deliver-conflicts-title", &ctx.t("delivery.step.conflicts"), None));
    let Ok(pipeline) = editing.shown().validate() else {
      return section.child(read_back("deliver-integrator-read-back", &ctx.t("delivery.step.integratorUnknown")));
    };
    let profiles = integrator_profiles(&pipeline);
    if profiles.is_empty() {
      return section.child(read_back("deliver-integrator-read-back", &ctx.t("delivery.step.noIntegrator")));
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
    let select = SelectField::new("deliver-integrator", ctx.t("delivery.step.resolveWith"), self.integrator.clone(), options)
      .leading(Icon::Bot)
      .render(ctx);
    section
      .child(control("deliver-integrator-row", &ctx.t("delivery.step.resolveWith"), select))
      .child(read_back("deliver-integrator-read-back", &ctx.t("delivery.step.integratorReadBack")))
  }
}

fn check_section(ctx: &Ctx, props: &DeliverProps, deliver: &DeliverStep) -> Column {
  let toggle = Toggle::new("deliver-recheck", ToggleKind::Switch, deliver.recheck(), ctx.t("delivery.step.recheck")).hide_label();
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
  let read_back_words = if deliver.recheck_locked() {
    ctx.t("delivery.step.recheckLocked")
  } else if !deliver.recheck() {
    ctx.t("delivery.step.recheckOff")
  } else {
    match check_tools(props) {
      Some(tools) => ctx.t_args("delivery.step.recheckReadBack", [("tools", tools)]),
      None => ctx.t("delivery.step.recheckNoTools"),
    }
  };
  section("deliver-check", SpacingSize::Xs)
    .child(section_title("deliver-check-title", &ctx.t("delivery.step.check"), None))
    .child(
      Row::new()
        .id("deliver-recheck-row")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("delivery.step.recheck"))
            .variant(ExtraTypography::Property.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(switch),
    )
    .child(read_back("deliver-recheck-read-back", &read_back_words))
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
      StepKind::Tool { tool: Some(tool) } => editing.opened.assignables.tool(tool).map(|choice| choice.name.to_string()),
      _ => None,
    })
    .collect();
  (!names.is_empty()).then(|| names.join(", "))
}

/// What a setting does, under its control, in line with the controls past their labels' column.
fn read_back(id: &str, words: &str) -> Row {
  control(
    id,
    "",
    Text::new(words)
      .id(format!("{id}-text"))
      .variant(DeliveryText::ReadBack.role())
      .width(Dimension::Pct(100.0)),
  )
}

fn remove_footer(ctx: &Ctx, props: &DeliverProps) -> Element {
  let state = props.state.clone();
  let remove = ActionButton::new("deliver-remove", ButtonKind::DangerGhost, ctx.t("delivery.step.remove"))
    .icon(Icon::Trash2)
    .on_click(move || {
      state.update(|editing| {
        if editing.viewing.is_none() {
          editing.edit.set_deliver(None);
        }
      });
      state.get_untracked().panels.close();
    });
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .child(Spacer::new().flex(1.0))
    .child(if props.locked { remove.disabled() } else { remove })
    .into()
}

/// A pipeline without a Deliver step: what that means, and *Add Deliver step*.
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
  parts::panel_body()
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("delivery.step.noneTitle")).variant(ExtraTypography::Strong.role()))
    .child(parts::note(&ctx.t("delivery.step.noneBody")))
    .child(Row::new().child(if props.locked { add.disabled() } else { add }))
    .into()
}

/// The words of the Deliver card and chip: its mode and method.
pub(super) fn executor_words(ctx: &Ctx, deliver: &DeliverStep) -> Arc<str> {
  let method = crate::screens::merge_queue::words::method(ctx, deliver.method());
  match deliver.mode() {
    DeliverMode::ApproveToMerge => ctx.t_args("delivery.step.cardApprove", [("method", method)]),
    DeliverMode::AutoMerge => ctx.t_args("delivery.step.cardAuto", [("method", method)]),
  }
}

/// The Deliver step's panel in the editor.
pub(super) fn panel(ctx: &mut Ctx, state: &State, locked: bool) -> Element {
  let editing = state.get();
  let props = DeliverProps {
    state: state.clone(),
    deliver: editing.shown().deliver().cloned(),
    locked,
  };
  ctx.mount_keyed::<DeliverInspector>(&format!("deliver-inspector-{}", editing.epoch), props)
}
