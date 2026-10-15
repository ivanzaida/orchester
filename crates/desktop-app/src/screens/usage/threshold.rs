//! The *Plan limits* card's menu (`Plan limits menu` `RvEnG`, `The … menu of Plan limits` in `p732q`): a
//! 28 square *…* that opens the warning threshold, a whole percentage of a window, 1–100, saved for
//! Orchester rather than a project because limits belong to accounts. It saves on change and says *Saved
//! for all projects*; the dashed line and its value move once the page has read it back.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::limits::WarningThreshold;
use orchester_workspace::{
  background,
  messages::{self, Message},
  usage::Dashboard,
};
use parking_lot::Mutex;

use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    icon_button::IconButton,
    message::message_text,
    popover::{PopoverSide, popover, popover_panel},
  },
};

/// The popover's width and the value's field (`Warning threshold` in `p732q`).
const WIDTH: f32 = 300.0;
const FIELD_WIDTH: f32 = 64.0;

/// The card's menu, for `dashboard`'s threshold; a save counts in `saved`.
pub fn menu(ctx: &mut Ctx, dashboard: &Dashboard, saved: &Signal<u64>) -> Element {
  ctx.mount_keyed::<ThresholdMenu>(
    "usage-threshold-menu",
    ThresholdProps {
      threshold: dashboard.threshold,
      saved: saved.clone(),
    },
  )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(WarningThreshold);

impl DevtoolsInspectable for Saved {}

type SaveAction = FutureAction<WarningThreshold, Saved, Message>;

#[derive(Clone)]
pub struct ThresholdProps {
  threshold: WarningThreshold,
  /// Counts saves, so the page reads the saved threshold.
  saved: Signal<u64>,
}

impl PartialEq for ThresholdProps {
  fn eq(&self, other: &Self) -> bool {
    self.threshold == other.threshold
  }
}

impl DevtoolsInspectable for ThresholdProps {}

pub struct ThresholdMenu {
  open: Signal<bool>,
  anchor: ElementRef,
  text: Signal<String>,
  /// A typed value that is not a whole percentage.
  invalid: Signal<bool>,
  save: SaveAction,
}

impl Component for ThresholdMenu {
  type Props = ThresholdProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ThresholdProps>().clone();
    let workspace = Services::of(ctx).workspace;
    let save: SaveAction = ctx.future_action(move |threshold: WarningThreshold| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.set_usage_warning_threshold(threshold))
          .await?
          .map(|()| Saved(threshold))
          .map_err(|error| {
            tracing::error!(%error, "cannot save the usage warning threshold");
            messages::store_error(&error)
          })
      }
    });
    let saved = props.saved.clone();
    ctx.watch(&save.state(), move |state| {
      if state.data.is_some() && !state.is_pending() && state.error.is_none() {
        saved.update(|count| *count += 1);
      }
    });
    let menu = Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
      text: ctx.signal(props.threshold.percent().to_string()),
      invalid: ctx.signal(false),
      save: save.clone(),
    };
    // Saves on change, whatever changed the field's text; an unchanged value is not saved again.
    let (invalid, last) = (menu.invalid.clone(), Arc::new(Mutex::new(props.threshold)));
    ctx.watch(&menu.text, move |typed: &String| match WarningThreshold::parse(typed) {
      Ok(threshold) => {
        invalid.set(false);
        let mut last = last.lock();
        if *last != threshold {
          *last = threshold;
          save.run(threshold);
        }
      }
      Err(_) => invalid.set(true),
    });
    menu
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = self.open.clone();
    let button = IconButton::new("usage-limits-menu", Icon::Ellipsis, ctx.t("usage.limits.menu"))
      .on_click(move || open.update(|open| *open = !*open));
    let button = if self.open.get() { button.active() } else { button }.render(ctx);
    let mut row = Row::new().ref_element(self.anchor.clone()).child(button);
    if self.open.get() {
      let panel = self.panel(ctx);
      row = row.child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::Below,
        panel,
      ));
    }
    row
  }
}

impl ThresholdMenu {
  fn panel(&self, ctx: &mut Ctx) -> Element {
    let field = FieldInput::new("usage-threshold", ctx.t("usage.threshold.title"), self.text.clone())
      .mono()
      .width(Dimension::Px(FIELD_WIDTH));
    let field = if self.invalid.get() { field.invalid() } else { field }.render(ctx);
    let state = self.save.state().get();
    let outcome: Option<Element> = match (&state.error, &state.data) {
      _ if self.invalid.get() => Some(field_error("usage-threshold-error", &ctx.t("usage.threshold.invalid")).into()),
      (Some(error), _) if !state.is_pending() => {
        Some(field_error("usage-threshold-error", &message_text(ctx, error)).into())
      }
      (None, Some(_)) if !state.is_pending() => Some(
        Row::new()
          .id("usage-threshold-saved")
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Check, IconSize::Sm, PaletteColor::Success))
          .child(
            Text::new(&ctx.t("usage.threshold.saved"))
              .variant(TypographyStyle::Caption)
              .color(PaletteColor::TextSecondary),
          )
          .into(),
      ),
      _ => None,
    };
    // A value that is not a percentage says so under the field; what saving did comes last.
    let (under_field, last) = if self.invalid.get() {
      (outcome, None)
    } else {
      (None, outcome)
    };
    let panel = popover_panel("usage-threshold-popover", WIDTH)
      .padding(ExtraSpacing::Inset.role())
      .child(Text::new(&ctx.t("usage.threshold.title")).variant(ExtraTypography::InspectorSection.role()))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(field)
          .child(Text::new(&ctx.t("usage.threshold.unit")).variant(TypographyStyle::Description)),
      )
      .with_children(under_field)
      .child(
        Text::new(&ctx.t("usage.threshold.hint"))
          .width(Dimension::Pct(100.0))
          .variant(TypographyStyle::Caption),
      )
      .with_children(last);
    panel.into()
  }
}
