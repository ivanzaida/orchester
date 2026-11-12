//! *Waits for* in an open step's *Run* group (`Waits for` `DyX2q` in `Pipelines · v2` P16b `P1853`), the one place a
//! step's dependencies are edited: a chip per step it names (its × stops waiting for it) and *Add*, which opens the
//! steps it may wait for (`Waits for menu` `A2Cyzn`). Only steps that do not wait for this one are offered, so no loop
//! is made here; a step it already waits for through another is ticked and faded with *through 04*, and *Wait for
//! everything before it* puts the step back under the stage before it. A tick changes the edit at once and keeps the
//! menu open; *Save* and *Cancel* act on the whole pipeline. The row shows for a step that waits for only part of the
//! stage before it, and for one whose row the person opened (*Wait for some steps only…*) or changed: it stays for
//! that step until the edit is saved or dropped ([`crate::screens::pipeline_hub::edit::Edit::show_waits`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{MoveError, MoveOffer, PipelineEdit, StepKey, WaitChoice, Waited};

use crate::{
  screens::{
    pipeline_hub::{edit::Edit, flow::number},
    team_hub::rows::{self, Sub},
  },
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    menu::{MenuItem, menu, menu_panel, separator},
    pressable::{Focus, inert, pressable},
    toggle::check_mark,
  },
};

/// The menu's width (`Waits for menu` `A2Cyzn`).
const MENU_WIDTH: f32 = 270.0;
const ID: &str = "pipeline-hub-step-waits";

/// A step as the row names it: its key, and its number and name in the flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Card {
  pub key: StepKey,
  pub number: usize,
  pub name: Arc<str>,
}

#[derive(Clone)]
pub(super) struct Props {
  pub edit: Edit,
  pub key: StepKey,
  /// What the panel shows, which the choices are read from.
  pub current: PipelineEdit,
  /// The steps of the flow, by number and name.
  pub cards: Vec<Card>,
  /// A save is under way or asks: nothing is changed now.
  pub locked: bool,
}

/// The props are equal when they say the same: the edit's handle is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.key, &self.current, &self.cards, self.locked) == (&other.key, &other.current, &other.cards, other.locked)
  }
}
impl DevtoolsInspectable for Props {}

/// The row of `props`, mounted per step.
pub(super) fn row(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("{ID}-host-{}", props.key);
  ctx.mount_keyed::<WaitsFor>(&key, props)
}

/// A change the edit refused, which the row says under its label until the next change.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Refusal(MoveError);
impl DevtoolsInspectable for Refusal {}

struct WaitsFor {
  open: Signal<bool>,
  anchor: ElementRef,
  refused: Signal<Option<Refusal>>,
}

impl Component for WaitsFor {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::new(),
      refused: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    if props.locked && self.open.get_untracked() {
      self.open.set(false);
    }
    // The row is drawn only for steps that have an order to draw, whose choices are always read; steps that lost it
    // since show no choice.
    let choices = props.current.wait_choices(&props.key).unwrap_or_default();
    let chips: Vec<Element> = choices
      .iter()
      .filter(|choice| choice.waited == Waited::Directly)
      .map(|choice| self.chip(ctx, &props, choice))
      .collect();
    let mut value = Row::new()
      .id(format!("{ID}-value"))
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    if chips.is_empty() {
      value = value.child(
        Text::new(&ctx.t("pipeline_hub.step.waitsFor.nothing"))
          .id(format!("{ID}-nothing"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted),
      );
    }
    value = value.with_children(chips).child(self.add(ctx, &props));
    if self.open.get() && !props.locked {
      value = value.child(self.popup(ctx, &props, &choices));
    }
    let mut sub = ctx.t("pipeline_hub.step.waitsFor.hint");
    if let Some(Refusal(error)) = self.refused.get() {
      sub = reason(ctx, &props, &error);
    }
    rows::row(
      &format!("{ID}-row"),
      &ctx.t("pipeline_hub.step.waitsFor.label"),
      Some(Sub::quiet(sub)),
      value,
    )
  }
}

impl WaitsFor {
  /// A step it names (`Chip · 04 Run checks` `erp8e`): its number in Geist Mono, its name and the × that stops waiting
  /// for it, faded with why when that would break a failure route.
  fn chip(&self, ctx: &Ctx, props: &Props, choice: &WaitChoice) -> Element {
    let card = card(props, &choice.key);
    let id = format!("{ID}-chip-{}", choice.key);
    let label = ctx.t_args("pipeline_hub.step.waitsFor.remove", [("name", card.name.as_ref())]);
    let cross = match (&choice.offer, props.locked) {
      (MoveOffer::Available, false) => {
        let change = self.change(props, &choice.key, false);
        Button::empty()
          .id(format!("{id}-remove"))
          .align_items(Alignment::Center)
          .focusable(false)
          .cursor(CursorIcon::Pointer)
          .describe("a11y_name", label)
          .on_click(move |event: MouseEvent| {
            event.stop_propagation();
            change();
          })
      }
      (offer, _) => {
        let faded = inert(Button::empty())
          .id(format!("{id}-remove"))
          .opacity(DISABLED_OPACITY)
          .describe("a11y_name", label)
          .describe("state", "unavailable");
        match offer {
          MoveOffer::Refused(error) => faded.describe("reason", reason(ctx, props, error)),
          MoveOffer::Available | MoveOffer::Unavailable => faded,
        }
      }
    }
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted));
    Row::new()
      .id(id.clone())
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .padding(
        Padding::new()
          .left(ExtraSpacing::Loose.role())
          .right(ExtraSpacing::Snug.role()),
      )
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(ExtraRadius::Full.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .describe("step", choice.key.to_string())
      .child(
        Text::new(&number(card.number))
          .id(format!("{id}-number"))
          .variant(ExtraTypography::MonoNote.role()),
      )
      .child(
        Text::new(&card.name)
          .id(format!("{id}-name"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      )
      .child(cross)
      .into()
  }

  /// *Add* (`Add` `oKPqz`), which opens or closes the menu.
  fn add(&self, ctx: &Ctx, props: &Props) -> Element {
    let open = self.open.clone();
    let button = ActionButton::new(
      format!("{ID}-add"),
      ButtonKind::Ghost,
      ctx.t("pipeline_hub.step.waitsFor.add"),
    )
    .icon(Icon::Plus)
    .element_ref(self.anchor.clone())
    .on_click(move || open.update(|open| *open = !*open));
    if props.locked { button.disabled() } else { button }.into()
  }

  /// The steps it may wait for, then *Wait for everything before it*.
  fn popup(&self, ctx: &mut Ctx, props: &Props, choices: &[WaitChoice]) -> Element {
    let mut panel = menu_panel(&format!("{ID}-menu")).width(Dimension::Px(MENU_WIDTH));
    for choice in choices {
      panel = panel.child(self.choice_row(ctx, props, choice));
    }
    panel = panel
      .child(separator(ctx))
      .child(self.everything(ctx, props).render(ctx, &self.open));
    menu(self.anchor.clone(), self.open.clone(), panel).into()
  }

  /// A step of the menu (`Menu item · 05 Review` `Eds7C`): its box, number and name, and *through 04* when it is waited
  /// for through another, which fades it. A tick or an untick changes the edit and keeps the menu open.
  fn choice_row(&self, ctx: &Ctx, props: &Props, choice: &WaitChoice) -> Element {
    let card = card(props, &choice.key);
    let id = format!("{ID}-choice-{}", choice.key);
    let on = choice.waited != Waited::No;
    let through = match &choice.waited {
      Waited::Through(by) => Some(ctx.t_args(
        "pipeline_hub.step.waitsFor.through",
        [("number", number(self::card(props, by).number))],
      )),
      Waited::Directly | Waited::No => None,
    };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(check_mark(on))
      .child(
        Text::new(&number(card.number))
          .id(format!("{id}-number"))
          .variant(ExtraTypography::MonoNote.role()),
      )
      .child(
        Text::new(&card.name)
          .id(format!("{id}-name"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .with_children(through.map(|through| {
        Text::new(&through)
          .id(format!("{id}-through"))
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
      }));
    let surface = match (&choice.offer, props.locked) {
      (MoveOffer::Available, false) => Focus::Keyboard
        .apply(pressable(self.change(props, &choice.key, !on)))
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .describe("state", "enabled"),
      (offer, _) => {
        let faded = inert(Button::empty())
          .opacity(DISABLED_OPACITY)
          .describe("state", "unavailable");
        match offer {
          MoveOffer::Refused(error) => faded.describe("reason", reason(ctx, props, error)),
          MoveOffer::Available | MoveOffer::Unavailable => faded,
        }
      }
    };
    surface
      .id(id)
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Sm)
      .describe("a11y_name", card.name)
      .describe("chosen", if on { "true" } else { "false" })
      .child(row)
      .into()
  }

  /// *Wait for everything before it*, faded with why when the step already waits for all of it or nothing runs before.
  fn everything(&self, ctx: &Ctx, props: &Props) -> MenuItem {
    let id = format!("{ID}-everything");
    let label = ctx.t("pipeline_hub.step.waitsFor.everything");
    let mut probe = props.current.clone();
    let item = match probe.wait_for_stage_before(&props.key) {
      Ok(true) => {
        let (edit, key, refused) = (props.edit.clone(), props.key.clone(), self.refused.clone());
        MenuItem::new(id, label, move || {
          let mut outcome = Ok(false);
          edit.change(|held| outcome = held.wait_for_stage_before(&key));
          settle(&edit, &key, &refused, outcome);
        })
      }
      Ok(false) if first_stage(props) => {
        MenuItem::unavailable(id, label, ctx.t("pipeline_hub.step.waitsFor.nothingBefore"))
      }
      Ok(false) => MenuItem::unavailable(id, label, ctx.t("pipeline_hub.step.waitsFor.waitsForAll")),
      Err(error) => MenuItem::unavailable(id, label, reason(ctx, props, &error)),
    };
    item.icon(Icon::RotateCcw).without_lock()
  }

  /// What makes the step wait for `dependency` (`waits`) or stop: the edit changes, and a refusal is said in the row.
  fn change(&self, props: &Props, dependency: &StepKey, waits: bool) -> impl Fn() + Send + Sync + 'static + use<> {
    let (edit, key, dependency, refused) = (
      props.edit.clone(),
      props.key.clone(),
      dependency.clone(),
      self.refused.clone(),
    );
    move || {
      let mut outcome = Ok(false);
      edit.change(|held| outcome = held.wait_for(&key, &dependency, waits));
      settle(&edit, &key, &refused, outcome);
    }
  }
}

/// After a change to what step `key` waits for: its row stays shown, and a refusal is kept to be said.
fn settle(edit: &Edit, key: &StepKey, refused: &Signal<Option<Refusal>>, outcome: Result<bool, MoveError>) {
  edit.show_waits(key);
  let now = outcome.err().map(Refusal);
  if refused.get_untracked() != now {
    refused.set(now);
  }
}

/// Whether the step stands in the first stage, which nothing runs before.
fn first_stage(props: &Props) -> bool {
  props
    .current
    .step(&props.key)
    .is_some_and(|step| step.depends_on.is_empty())
}

/// The number and name of step `key`; a step the flow does not number keeps its key as its name.
fn card(props: &Props, key: &StepKey) -> Card {
  props
    .cards
    .iter()
    .find(|card| card.key == *key)
    .cloned()
    .unwrap_or_else(|| Card {
      key: key.clone(),
      number: 0,
      name: Arc::from(key.as_str()),
    })
}

/// Why a change to what the step waits for was refused, naming steps by number and name.
fn reason(ctx: &Ctx, props: &Props, error: &MoveError) -> Arc<str> {
  let label = |key: &StepKey| {
    let card = card(props, key);
    ctx
      .t_args(
        "pipeline_hub.flow.move.step",
        [("number", number(card.number)), ("name", card.name.to_string())],
      )
      .to_string()
  };
  match error {
    MoveError::RetryTargetNotUpstream { step, target } => ctx.t_args(
      "pipeline_hub.flow.move.reason.returnsTo",
      [("step", label(step)), ("target", label(target))],
    ),
    MoveError::WaitsForIt { dependency, .. } => ctx.t_args(
      "pipeline_hub.flow.move.reason.waitsForIt",
      [("step", label(dependency))],
    ),
    MoveError::UnknownStep(_) => ctx.t("pipeline_hub.flow.move.reason.unknownStep"),
    MoveError::WithItself(_) => ctx.t("pipeline_hub.flow.move.reason.withItself"),
    MoveError::Broken(_) => ctx.t("pipeline_hub.flow.move.reason.broken"),
  }
}
