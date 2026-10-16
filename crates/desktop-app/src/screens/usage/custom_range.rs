//! *Custom…* (`Custom… · From / To under the period control` in `p732q`): a popover under the period
//! control with the From and To days and *Show Sep 1 – Sep 30*, which shows the range once both days read
//! and it does not end before it starts; until then it says why and shows nothing.

use chrono::NaiveDate;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  time::Timestamp,
  usage::{Period, local_date},
};
use orchester_workspace::usage::{CustomRangeError, DAY_FORMAT, LONGEST_DAYS, custom_range};

use super::{Requery, toolbar::ToolbarProps, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    popover::popover_panel,
  },
};

/// The popover's width (`Custom range` in `p732q`).
const WIDTH: f32 = 300.0;

/// The popover, open under `anchor` while `open` holds.
pub fn popover(ctx: &mut Ctx, props: &ToolbarProps, (open, anchor): &(Signal<bool>, ElementRef)) -> Element {
  let range = props.query.range(local_date(Timestamp::now()));
  let content = ctx.mount_keyed::<CustomRange>(
    "usage-custom-range",
    RangeProps {
      requery: props.requery.clone(),
      query: props.query.clone(),
      first: range.first(),
      last: range.last(),
      open: open.clone(),
    },
  );
  Popup::new(anchor.clone(), content)
    .open(open.clone())
    .placement(Placement::BottomEnd)
    .offset(0.0, ExtraSpacing::Snug.value())
    .into()
}

#[derive(Clone)]
struct RangeProps {
  requery: Requery,
  query: orchester_workspace::usage::UsageQuery,
  first: NaiveDate,
  last: NaiveDate,
  open: Signal<bool>,
}

impl PartialEq for RangeProps {
  fn eq(&self, other: &Self) -> bool {
    self.query == other.query && self.first == other.first && self.last == other.last
  }
}

impl DevtoolsInspectable for RangeProps {}

struct CustomRange {
  from: Signal<String>,
  to: Signal<String>,
}

impl Component for CustomRange {
  type Props = RangeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<RangeProps>();
    let (from, to) = (
      props.first.format(DAY_FORMAT).to_string(),
      props.last.format(DAY_FORMAT).to_string(),
    );
    Self {
      from: ctx.signal(from),
      to: ctx.signal(to),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RangeProps>().clone();
    let range = custom_range(&self.from.get(), &self.to.get());
    let reversed = matches!(range, Err(CustomRangeError::Reversed | CustomRangeError::TooLong));
    let from = FieldInput::new("usage-custom-from", ctx.t("usage.custom.from"), self.from.clone()).mono();
    let to = FieldInput::new("usage-custom-to", ctx.t("usage.custom.to"), self.to.clone()).mono();
    let to = if reversed { to.invalid() } else { to };
    let fields = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        field(
          "usage-custom-from-field",
          Some(&ctx.t("usage.custom.from")),
          from.render(ctx),
          None,
        )
        .flex(1.0),
      )
      .child(
        field(
          "usage-custom-to-field",
          Some(&ctx.t("usage.custom.to")),
          to.render(ctx),
          None,
        )
        .flex(1.0),
      );
    let note: Element = match range {
      Ok(_) => Text::new(&ctx.t("usage.custom.hint"))
        .id("usage-custom-hint")
        .variant(TypographyStyle::Caption)
        .into(),
      Err(CustomRangeError::Reversed) => field_error("usage-custom-error", &ctx.t("usage.custom.reversed")).into(),
      Err(CustomRangeError::Unreadable) => field_error("usage-custom-error", &ctx.t("usage.custom.unreadable")).into(),
      Err(CustomRangeError::TooLong) => {
        let text = ctx.t_args("usage.custom.tooLong", [("days", LONGEST_DAYS.to_string())]);
        field_error("usage-custom-error", &text).into()
      }
    };
    let show: Element = match range {
      Ok(range) => {
        let today = local_date(Timestamp::now());
        let (requery, query, open) = (
          props.requery,
          props.query.with_period(Period::Custom(range), today),
          props.open,
        );
        ActionButton::new(
          "usage-custom-show",
          ButtonKind::Primary,
          ctx.t_args("usage.custom.show", [("range", &*words::range(ctx, range))]),
        )
        .in_form()
        .on_click(move || {
          open.set(false);
          requery.show(query.clone());
        })
        .into()
      }
      Err(_) => ActionButton::new(
        "usage-custom-show",
        ButtonKind::Primary,
        ctx.t("usage.custom.showNothing"),
      )
      .disabled()
      .into(),
    };
    popover_panel("usage-custom", WIDTH)
      .padding(ExtraSpacing::Inset.role())
      .child(Text::new(&ctx.t("usage.custom.title")).variant(ExtraTypography::InspectorSection.role()))
      .child(fields)
      .child(note)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::End)
          .justify(Justify::End)
          .child(show),
      )
  }
}
