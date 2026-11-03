//! The plan the Orchestrator answered with in plan-only mode (`Plan` `TwyKf` in `12b` `X7BYz`; its states in `12d`
//! `y79oJ7`). While it waits it is a card as the decision card is (`surface_raised` in a 1 `border_strong` stroke,
//! radius 10, 12 over and under and 14 at its sides, its parts 10 apart): its head — a 14 `list-checks`, *Plan · N
//! steps* in 13/600, *nothing done yet* in 12 `text_muted` and at its end the *Waits for you · time* chip, or *Being
//! changed* while the composer changes it; its steps as 32 high rows between `border` lines, 4 in at their sides, 10
//! apart: the number in Geist Mono 11 `text_muted`, a 13 icon of the kind (`play`, `file-plus`, `sliders-horizontal`),
//! what will happen in 12.5 `text_primary` and its target in 12 `text_muted`; *Left out* and *Uses* as the decision
//! card's facts; and *Approve and do it*, *Change it*, *Dismiss*, with a lock and *Only you can approve* at the end.
//! Only a person approves it: lurq's synthetic input is refused, visibly. Decided, it folds into a line on the `hover`
//! fill (`t5sBl`): approved, replaced by the next plan or dismissed, *by you*, when, and *Show plan*, which opens its
//! steps under it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ChatId,
  orchestrator::{ChatPlan, PlanState, PlanStep, StepKind},
  run::Answerer,
  time::Timestamp,
};
use orchester_workspace::{display, orchestrator::send::SendError, workspace::OpenProject};

use super::{ApproveAction, Approving, Changing, DismissAction, approve_action, dismiss_action};
use crate::{
  screens::orchestrator::{
    decision::{self, FACT_KEY_WIDTH},
    line::{Chip, ChipTone, LineLink, chip_element, link_element, nested},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    tinted_note::{person_only, tinted_note},
  },
};

/// A step's row (`h4Uqx`).
const STEP_HEIGHT: f32 = 32.0;
/// A folded plan's line (`t5sBl`).
const FOLDED_HEIGHT: f32 = 32.0;

/// Where a card's decisions go: the project and its conversation, and what *Change it* does.
#[derive(Clone)]
pub(in crate::screens::orchestrator) struct Deciding {
  pub(in crate::screens::orchestrator) project: OpenProject,
  pub(in crate::screens::orchestrator) chat: ChatId,
  pub(in crate::screens::orchestrator) change: Arc<dyn Fn(Changing) + Send + Sync>,
}

#[derive(Clone)]
pub(in crate::screens::orchestrator) struct PlanProps {
  pub(in crate::screens::orchestrator) id: String,
  pub(in crate::screens::orchestrator) plan: ChatPlan,
  /// Its entry in the conversation.
  pub(in crate::screens::orchestrator) seq: i64,
  /// Which plan of the conversation it is, from 1.
  pub(in crate::screens::orchestrator) number: usize,
  /// When it was proposed.
  pub(in crate::screens::orchestrator) at: Timestamp,
  /// The composer is changing it.
  pub(in crate::screens::orchestrator) changing: bool,
  /// A turn of the conversation still thinks: *Approve and do it* waits for it to end.
  pub(in crate::screens::orchestrator) answering: bool,
  /// `None` while the conversation is not kept: nothing is decided then.
  pub(in crate::screens::orchestrator) deciding: Option<Deciding>,
}

/// Compared by what it shows; *Change it* is the screen's.
impl PartialEq for PlanProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.plan == other.plan
      && (self.seq, self.number, self.at) == (other.seq, other.number, other.at)
      && (self.changing, self.answering) == (other.changing, other.answering)
      && self.deciding.as_ref().map(|deciding| deciding.chat) == other.deciding.as_ref().map(|deciding| deciding.chat)
  }
}

impl DevtoolsInspectable for PlanProps {}

pub(in crate::screens::orchestrator) struct PlanCard {
  approve: ApproveAction,
  dismiss: DismissAction,
  /// Synthetic input tried *Approve and do it*.
  refused: Signal<bool>,
  /// A folded plan's steps are open under its line.
  open: Signal<bool>,
}

impl Component for PlanCard {
  type Props = PlanProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      approve: approve_action(ctx),
      dismiss: dismiss_action(ctx),
      refused: ctx.signal(false),
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PlanProps>().clone();
    let count = props.plan.steps().len().to_string();
    let folded = match props.plan.state() {
      PlanState::Waits => return self.card(ctx, &props),
      PlanState::Approved { at } => (
        (Icon::CircleCheck, PaletteColor::Success, "approved"),
        ctx.t_args("orchestrator.plan.approved", [("steps", count.as_str())]),
        at,
      ),
      PlanState::Replaced { at } => (
        (Icon::History, PaletteColor::TextMuted, "replaced"),
        ctx.t_args("orchestrator.plan.replaced", [("plan", (props.number + 1).to_string())]),
        at,
      ),
      PlanState::Dismissed { at } => (
        (Icon::CircleSlash, PaletteColor::TextMuted, "dismissed"),
        ctx.t("orchestrator.plan.dismissed"),
        at,
      ),
    };
    self.folded(ctx, &props, folded)
  }
}

impl PlanCard {
  /// The card of a plan that waits.
  fn card(&self, ctx: &mut Ctx, props: &PlanProps) -> Element {
    let id = props.id.as_str();
    let refused = self
      .refused
      .get()
      .then(|| person_only(format!("{id}-person-only"), &ctx.t("orchestrator.plan.personOnly")));
    let failed = self
      .approve
      .state()
      .get()
      .error
      .map(|error| {
        let (SendError::NotSent(why) | SendError::Unconfirmed(why)) = error;
        why
      })
      .or_else(|| self.dismiss.state().get().error)
      .map(|why| {
        tinted_note(
          format!("{id}-error"),
          (Icon::CircleAlert, PaletteColor::Danger),
          &message_text(ctx, &why),
        )
      });
    let actions = self.actions(ctx, props);
    decision::frame(id, "plan", false)
      .child(head(ctx, props))
      .child(steps(&format!("{id}-steps"), &props.plan))
      .with_children(facts(ctx, id, &props.plan))
      .with_children(refused)
      .with_children(failed)
      .child(actions)
      .into()
  }

  /// *Approve and do it*, *Change it* and *Dismiss*, then a lock and *Only you can approve*; none takes a press while
  /// a decision is under way, or before the conversation is kept, and *Approve and do it* none while a turn of the
  /// conversation still thinks.
  fn actions(&self, ctx: &mut Ctx, props: &PlanProps) -> Row {
    let id = props.id.as_str();
    let approve = ActionButton::new(
      format!("{id}-approve"),
      ButtonKind::Primary,
      ctx.t("orchestrator.plan.approve"),
    )
    .icon(Icon::Check);
    let change = ActionButton::new(
      format!("{id}-change"),
      ButtonKind::Secondary,
      ctx.t("orchestrator.plan.change"),
    )
    .icon(Icon::Pencil);
    let dismiss = ActionButton::new(
      format!("{id}-dismiss"),
      ButtonKind::Ghost,
      ctx.t("orchestrator.plan.dismiss"),
    );
    let approving = self.approve.state().get().is_pending();
    let pending = approving || self.dismiss.state().get().is_pending();
    let buttons: [ActionButton; 3] = match props.deciding.clone().filter(|_| !pending) {
      None if approving => [approve.loading(), change.disabled(), dismiss.disabled()],
      None => [approve.disabled(), change.disabled(), dismiss.disabled()],
      Some(deciding) => {
        let [approve, change, dismiss] = self.pressable([approve, change, dismiss], props, deciding);
        [
          if props.answering { approve.disabled() } else { approve },
          change,
          dismiss,
        ]
      }
    };
    Row::new()
      .id(format!("{id}-actions"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(buttons)
      .child(Spacer::new().flex(1.0))
      .child(
        Row::new()
          .id(format!("{id}-person"))
          .flex_shrink(0.0)
          .spacing(ExtraSpacing::Pair.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted))
          .child(
            Text::new(&ctx.t("orchestrator.plan.onlyYou"))
              .variant(ExtraTypography::Note.role())
              .nowrap(),
          ),
      )
  }

  /// The three buttons, each with what its press does in `deciding`'s conversation.
  fn pressable(
    &self,
    [approve, change, dismiss]: [ActionButton; 3],
    props: &PlanProps,
    deciding: Deciding,
  ) -> [ActionButton; 3] {
    let (action, refused, seq) = (self.approve.clone(), self.refused.clone(), props.seq);
    let approving = Approving {
      project: deciding.project.clone(),
      chat: deciding.chat,
      seq,
      by: Answerer::Person,
    };
    let approve = approve.on_click(move || {
      if is_synthetic_input() {
        tracing::warn!("synthetic input reached the approval of a plan of the Orchestrator; not taken");
        refused.set(true);
        return;
      }
      action.run(approving.clone());
    });
    let changing = Changing {
      seq,
      steps: props.plan.steps().len(),
      at: props.at,
    };
    let change = change.on_click(move || (deciding.change)(changing));
    let (action, root) = (self.dismiss.clone(), deciding.project.repo_root);
    let dismiss = dismiss.on_click(move || action.run((root.clone(), seq)));
    [approve, change, dismiss]
  }

  /// The line a decided plan folds into: `glyph` in `color`, what became of it, *by you*, when and *Show plan*; its
  /// steps under it while open.
  fn folded(
    &self,
    ctx: &mut Ctx,
    props: &PlanProps,
    ((glyph, color, outcome), what, at): ((Icon, PaletteColor, &'static str), Arc<str>, Timestamp),
  ) -> Element {
    let id = props.id.as_str();
    let open = self.open.get();
    let toggle = self.open.clone();
    let link = LineLink {
      label: ctx.t(if open {
        "orchestrator.plan.hidePlan"
      } else {
        "orchestrator.plan.showPlan"
      }),
      press: Press::run(move || toggle.update(|open| *open = !*open)),
    };
    let line = Row::new()
      .id(id.to_owned())
      .describe("outcome", outcome)
      .describe("open", open.to_string())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FOLDED_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .background(ExtraColor::Hover.role())
      .rounded(RadiusSize::Md)
      .child(icon(glyph, IconSize::Base, color).flex_shrink(0.0))
      .child(
        Text::new(&what)
          .id(format!("{id}-title"))
          .variant(OrchestratorText::Line.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(
        Text::new(&ctx.t("orchestrator.line.byYou"))
          .id(format!("{id}-target"))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap()
          .flex_shrink(0.0),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&display::clock(at))
          .id(format!("{id}-time"))
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
          .flex_shrink(0.0),
      )
      .child(link_element(&format!("{id}-show"), link));
    let shown = open.then(|| {
      nested(
        &format!("{id}-plan"),
        Column::new()
          .width(Dimension::Pct(100.0))
          .child(steps(&format!("{id}-steps"), &props.plan)),
      )
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(line)
      .with_children(shown)
      .into()
  }
}

/// The card's head: the glyph, *Plan · N steps*, *nothing done yet* and the chip of where it stands.
fn head(ctx: &mut Ctx, props: &PlanProps) -> Row {
  let id = props.id.as_str();
  let count = props.plan.steps().len().to_string();
  let chip = if props.changing {
    Chip::new(ctx.t("orchestrator.plan.beingChanged"), ChipTone::Neutral)
  } else {
    let time = display::clock(props.at);
    Chip::new(
      ctx.t_args("orchestrator.plan.waits", [("time", time.as_str())]),
      ChipTone::Warning,
    )
  };
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::ListChecks, IconSize::Md, PaletteColor::TextSecondary).flex_shrink(0.0))
    .child(
      Text::new(&ctx.t_args("orchestrator.plan.title", [("steps", count.as_str())]))
        .id(format!("{id}-title"))
        .variant(OrchestratorText::Question.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&ctx.t("orchestrator.plan.nothingDone"))
        .id(format!("{id}-note"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .child(chip_element(ctx, &format!("{id}-state"), &chip))
}

/// The plan's steps, named `id`: a `border` line over them and under each.
fn steps(id: &str, plan: &ChatPlan) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(
      plan
        .steps()
        .iter()
        .enumerate()
        .map(|(index, step)| step_row(&format!("{id}-{index}"), index + 1, step)),
    )
}

/// Step `number` of a plan.
fn step_row(id: &str, number: usize, step: &PlanStep) -> Row {
  let glyph = match step.kind() {
    StepKind::Run => Icon::Play,
    StepKind::Draft => Icon::FilePlus,
    StepKind::Setting => Icon::SlidersHorizontal,
    StepKind::Other => Icon::ListChecks,
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STEP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&number.to_string())
        .id(format!("{id}-number"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary).flex_shrink(0.0))
    .child(
      Text::new(step.action())
        .id(format!("{id}-action"))
        .variant(OrchestratorText::Line.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(step.target().map(|target| {
      Text::new(target)
        .id(format!("{id}-target"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
    }))
}

/// *Left out* and *Uses*, as the decision card's facts; nothing when the plan says neither.
fn facts(ctx: &Ctx, id: &str, plan: &ChatPlan) -> Option<Column> {
  let rows: Vec<Row> = [
    ("left-out", "orchestrator.plan.leftOut", plan.left_out()),
    ("uses", "orchestrator.plan.uses", plan.uses()),
  ]
  .into_iter()
  .filter_map(|(suffix, key, value)| {
    let value = value?;
    Some(
      Row::new()
        .id(format!("{id}-fact-{suffix}"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(
          Text::new(&ctx.t(key))
            .variant(OrchestratorText::FactKey.role())
            .width(Dimension::Px(FACT_KEY_WIDTH))
            .flex_shrink(0.0),
        )
        .child(
          Text::new(value)
            .variant(OrchestratorText::FactValue.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        ),
    )
  })
  .collect();
  (!rows.is_empty()).then(|| {
    Column::new()
      .id(format!("{id}-facts"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .with_children(rows)
  })
}
