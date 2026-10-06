//! The Plan limits card's menu (`e5PbG`): a 28 × 28 *…* button that opens the warning threshold, a
//! whole percentage of a window, 1–100, saved for Orchester rather than a project because limits
//! belong to accounts. It saves on change and says *Saved for all projects*; the dashed line and
//! its label move at once.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, OutsidePress, Placement},
    events::TextInputEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::limits::WarningThreshold;
use parking_lot::Mutex;

use super::load::save_threshold;
use crate::{
  background,
  messages::Message,
  theme::{ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::{
    field::{field_error, text_input},
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
  },
  workspace::Workspace,
};

/// The menu button (`e5PbG`), the popover and the value field.
const BUTTON_SIZE: f32 = 28.0;
const POPOVER_WIDTH: f32 = 320.0;
const FIELD_WIDTH: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(WarningThreshold);

impl lurq::app::component::DevtoolsInspectable for Saved {}

type SaveAction = FutureAction<WarningThreshold, Saved, Message>;

#[derive(Clone)]
pub struct ThresholdProps {
  pub threshold: WarningThreshold,
  pub workspace: Workspace,
  /// Counts saves, so the dashboard reads the saved threshold.
  pub saved: Signal<u64>,
}

impl PartialEq for ThresholdProps {
  fn eq(&self, other: &Self) -> bool {
    self.threshold == other.threshold && self.workspace == other.workspace
  }
}

impl lurq::app::component::DevtoolsInspectable for ThresholdProps {}

pub struct ThresholdMenu {
  open: Signal<bool>,
  anchor: ElementRef,
  text: Signal<String>,
  /// A typed value that is not a whole percentage.
  invalid: Signal<bool>,
}

impl Component for ThresholdMenu {
  type Props = ThresholdProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ThresholdProps>().clone();
    let action = save_action(ctx, props.workspace.clone());
    let saved = props.saved.clone();
    ctx.watch(&action.state(), move |state| {
      if state.data.is_some() && !state.is_pending() && state.error.is_none() {
        saved.update(|count| *count += 1);
      }
    });
    let menu = Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
      text: ctx.signal(props.threshold.percent().to_string()),
      invalid: ctx.signal(false),
    };
    // Saves on change, whatever changed the field's text; an unchanged value is not saved again.
    let (invalid, last) = (menu.invalid.clone(), Mutex::new(props.threshold));
    ctx.watch(&menu.text, move |typed: &String| match WarningThreshold::parse(typed) {
      Ok(threshold) => {
        invalid.set(false);
        let mut last = last.lock();
        if *last != threshold {
          *last = threshold;
          action.run(threshold);
        }
      }
      Err(_) => invalid.set(true),
    });
    menu
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let workspace = ctx.props::<ThresholdProps>().workspace.clone();
    let action = save_action(ctx, workspace);
    let open = self.open.clone();
    let button = pressable(
      surface()
        .id("usage-limits-menu")
        .size(Dimension::Px(BUTTON_SIZE), Dimension::Px(BUTTON_SIZE))
        .background(PaletteColor::SurfacePanel)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .rounded(RadiusSize::Sm)
        .align_items(Alignment::Center)
        .justify(Justify::Center),
      move || open.set(!open.get_untracked()),
    )
    .child(icon(Icon::Ellipsis, IconSize::Small, PaletteColor::TextSecondary));
    let mut row = Row::new().ref_element(self.anchor.clone()).child(button);
    if self.open.get() {
      row = row.child(
        Popup::new(self.anchor.clone(), self.popover(ctx, &action))
          .open(self.open.clone())
          .placement(Placement::BottomEnd)
          .outside_press(OutsidePress::PassThrough),
      );
    }
    row
  }
}

impl ThresholdMenu {
  fn popover(&self, ctx: &Ctx, action: &SaveAction) -> Element {
    let text = self.text.clone();
    let field = text_input(
      ctx,
      "usage-threshold",
      self.text.clone(),
      self.invalid.get(),
      ExtraTypography::MonoField.role(),
    )
    .width(Dimension::Px(FIELD_WIDTH))
    .on_input(move |event: TextInputEvent| text.set(event.new_value));
    let state = action.state().get();
    let outcome = match (&state.error, &state.data) {
      _ if self.invalid.get() => Some(field_error(&ctx.t("usage.threshold.invalid"))),
      (Some(error), _) if !state.is_pending() => Some(field_error(&message_text(ctx, error))),
      (None, Some(_)) if !state.is_pending() => Some(
        Row::new()
          .spacing(ExtraSpacing::Tight.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Check, IconSize::Small, PaletteColor::TextPrimary))
          .child(Text::new(&ctx.t("usage.threshold.saved")).variant(TypographyStyle::Caption))
          .into(),
      ),
      _ => None,
    };
    Column::new()
      .id("usage-threshold-popover")
      .width(Dimension::Px(POPOVER_WIDTH))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(ExtraRadius::SelectMenu.role())
      .box_shadow(ExtraShadow::Popover.role())
      .child(Text::new(&ctx.t("usage.threshold.title")).variant(TypographyStyle::Label))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(field)
          .child(Text::new(&ctx.t("usage.threshold.unit")).variant(TypographyStyle::Caption)),
      )
      .child(Text::new(&ctx.t("usage.threshold.hint")).variant(ExtraTypography::Hint.role()))
      .with_children(outcome)
      .into()
  }
}

fn save_action(ctx: &mut Ctx, workspace: Workspace) -> SaveAction {
  ctx.future_action(move |threshold: WarningThreshold| {
    let workspace = workspace.clone();
    async move {
      background::run(move || save_threshold(&workspace, threshold))
        .await?
        .map(Saved)
    }
  })
}
