//! *Custom…* (`e5PbG`): From and To date fields and *Show Sep 1 – Sep 30*, which shows the range
//! once both dates are read and the range does not end before it starts.

use chrono::NaiveDate;
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  time::Timestamp,
  usage::{DayRange, Period, local_date},
};

use super::{format, state::Controls};
use crate::{
  theme::{ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, text_input},
  },
};

/// The popover's width (`e5PbG`).
const POPOVER_WIDTH: f32 = 300.0;
/// How dates are typed.
const DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Clone)]
pub struct CustomRangeProps {
  pub controls: Signal<Controls>,
  pub open: Signal<bool>,
  pub range: DayRange,
}

impl PartialEq for CustomRangeProps {
  fn eq(&self, other: &Self) -> bool {
    self.range == other.range
  }
}

impl lurq::app::component::DevtoolsInspectable for CustomRangeProps {}

pub struct CustomRange {
  from: Signal<String>,
  to: Signal<String>,
}

impl Component for CustomRange {
  type Props = CustomRangeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let range = ctx.props::<CustomRangeProps>().range;
    Self {
      from: ctx.signal(range.first().format(DATE_FORMAT).to_string()),
      to: ctx.signal(range.last().format(DATE_FORMAT).to_string()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CustomRangeProps>().clone();
    let read = |text: &str| NaiveDate::parse_from_str(text.trim(), DATE_FORMAT).ok();
    let (from, to) = (read(&self.from.get()), read(&self.to.get()));
    let range = match (from, to) {
      (Some(from), Some(to)) => DayRange::new(from, to).map_err(|_| "usage.custom.reversed"),
      _ => Err("usage.custom.unreadable"),
    };
    let show = match range {
      Ok(range) => {
        let (controls, open) = (props.controls, props.open);
        ActionButton::new(ctx.t_args("usage.custom.show", [("range", &*format::range(ctx, range))]))
          .primary()
          .id("usage-custom-show")
          .in_form()
          .on_click(move || {
            let today = local_date(Timestamp::now());
            controls.update(|controls| *controls = controls.with_period(Period::Custom(range), today));
            open.set(false);
          })
          .render(ctx)
      }
      Err(_) => ActionButton::new(ctx.t("usage.custom.showNothing"))
        .primary()
        .id("usage-custom-show")
        .disabled()
        .render(ctx),
    };
    Column::new()
      .id("usage-custom")
      .width(Dimension::Px(POPOVER_WIDTH))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(ExtraRadius::SelectMenu.role())
      .box_shadow(ExtraShadow::Popover.role())
      .child(Text::new(&ctx.t("usage.custom.title")).variant(TypographyStyle::Label))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(date_field(ctx, "usage-custom-from", "usage.custom.from", &self.from))
          .child(date_field(ctx, "usage-custom-to", "usage.custom.to", &self.to)),
      )
      .child(Text::new(&ctx.t("usage.custom.hint")).variant(ExtraTypography::Hint.role()))
      .with_children(range.err().map(|key| field_error(&ctx.t(key))))
      .child(show)
  }
}

fn date_field(ctx: &Ctx, id: &'static str, label: &str, value: &Signal<String>) -> Element {
  Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Field.role())
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::FieldLabel))
    .child(text_input(
      ctx,
      id,
      value.clone(),
      false,
      ExtraTypography::MonoField.role(),
    ))
    .into()
}
