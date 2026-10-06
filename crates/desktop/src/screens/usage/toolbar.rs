//! The pinned toolbar (`C2pTYF`, `e5PbG`): on a project's page the title *Usage* and its
//! description; the period control (*Today* · *7 days* · *30 days* · *Custom…*, 36 high) and *Group
//! by* (`ZGwlv`, *By hour* offered up to seven days); under them the filter chips and the range
//! read-back with the period it is compared with. All projects shows the controls under its tabs,
//! with *Project* first and the number of projects in the read-back (`p4eVV6`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, OutsidePress, Placement},
    theme::{BorderSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Select, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  time::Timestamp,
  usage::{DayRange, Grouping, Period, local_date},
};

use super::{
  custom_range::{CustomRange, CustomRangeProps},
  filters::{FilterChips, FilterProps},
  format,
  load::Scope,
  state::{Controls, Dashboard},
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    field::select_style,
    segments::{SegmentSize, segments},
  },
};

/// The height of the period control and *Group by* (`C2pTYF`).
const CONTROL_HEIGHT: f32 = 36.0;

#[derive(Clone)]
pub struct ToolbarProps {
  pub controls: Signal<Controls>,
  pub dashboard: Option<Arc<Dashboard>>,
  pub range: DayRange,
  pub scope: Scope,
}

impl PartialEq for ToolbarProps {
  fn eq(&self, other: &Self) -> bool {
    self.dashboard == other.dashboard && self.range == other.range && self.scope == other.scope
  }
}

impl lurq::app::component::DevtoolsInspectable for ToolbarProps {}

/// A period control choice: a preset, or the custom range being chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
  Preset(Period),
  Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GroupingChoice(Grouping);

impl lurq::app::component::DevtoolsInspectable for GroupingChoice {}

pub struct UsageToolbar {
  grouping: Signal<GroupingChoice>,
  custom: Signal<bool>,
  custom_anchor: ElementRef,
}

impl Component for UsageToolbar {
  type Props = ToolbarProps;

  fn create(ctx: &mut Ctx) -> Self {
    let toolbar = Self {
      grouping: ctx.signal(GroupingChoice(Grouping::Day)),
      custom: ctx.signal(false),
      custom_anchor: ElementRef::default(),
    };
    // Registered once; the screen keeps the same controls signal for the toolbar's lifetime.
    let controls = ctx.props::<ToolbarProps>().controls.clone();
    ctx.watch(&toolbar.grouping, move |choice| {
      if controls.get_untracked().grouping != choice.0 {
        controls.update(|controls| controls.grouping = choice.0);
      }
    });
    toolbar
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolbarProps>().clone();
    let controls = props.controls.get();
    if self.grouping.get_untracked().0 != controls.grouping_for(props.range) {
      self.grouping.set(GroupingChoice(controls.grouping_for(props.range)));
    }
    let controls_row = Row::new()
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(self.period(ctx, &props))
      .child(self.group_by(ctx, props.range));
    let first = match &props.scope {
      Scope::Project(_) => Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(
          Column::new()
            .flex(1.0)
            .spacing(ExtraSpacing::Inline.role())
            .child(Text::new(&ctx.t("usage.title")).variant(ExtraTypography::ScreenTitle.role()))
            .child(Text::new(&ctx.t("usage.description")).variant(TypographyStyle::Description)),
        )
        .child(controls_row),
      Scope::All => Row::new().width(Dimension::Pct(100.0)).child(controls_row),
    };
    let filters = ctx.mount_keyed::<FilterChips>(
      "usage-filters",
      FilterProps {
        controls: props.controls.clone(),
        dashboard: props.dashboard.clone(),
      },
    );
    Column::new()
      .id("usage-toolbar")
      .width(Dimension::Pct(100.0))
      .padding_top(match props.scope {
        Scope::Project(_) => ExtraSpacing::HeadingTop.role(),
        Scope::All => SpacingSize::Md,
      })
      .padding_horizontal(props.scope.inset())
      .padding_bottom(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(first)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .align_items(Alignment::Center)
          .child(filters)
          .child(Spacer::new().flex(1.0))
          .child(readback(ctx, &props)),
      )
      .with_children(self.custom_popup(ctx, &props))
  }
}

impl UsageToolbar {
  /// *Today* · *7 days* · *30 days* · *Custom…*; the chosen one on `$hover`.
  fn period(&self, ctx: &Ctx, props: &ToolbarProps) -> Element {
    let controls = props.controls.get();
    let chosen = match controls.period {
      Period::Custom(_) => Choice::Custom,
      preset => Choice::Preset(preset),
    };
    let (signal, custom) = (props.controls.clone(), self.custom.clone());
    let control = segments(
      "usage-period",
      &chosen,
      vec![
        (Choice::Preset(Period::Today), ctx.t("usage.period.today")),
        (Choice::Preset(Period::Week), ctx.t("usage.period.week")),
        (Choice::Preset(Period::Month), ctx.t("usage.period.month")),
        (Choice::Custom, ctx.t("usage.period.custom")),
      ],
      SegmentSize::Large,
      move |choice| match choice {
        Choice::Preset(period) => {
          let today = local_date(Timestamp::now());
          signal.update(|controls| *controls = controls.with_period(period, today));
          custom.set(false);
        }
        Choice::Custom => custom.set(true),
      },
    );
    Row::new().ref_element(self.custom_anchor.clone()).child(control).into()
  }

  /// *Group by* (`ZGwlv`): *By hour* only for ranges up to seven days.
  fn group_by(&self, ctx: &Ctx, range: DayRange) -> Element {
    let mut options = Vec::new();
    if range.offers_hours() {
      options.push((GroupingChoice(Grouping::Hour), ctx.t("usage.groupBy.hour")));
    }
    options.push((GroupingChoice(Grouping::Day), ctx.t("usage.groupBy.day")));
    Element::from(
      Select::new(self.grouping.clone())
        .options(options)
        .style(select_style(ctx, CONTROL_HEIGHT)),
    )
    .id("usage-group-by")
  }

  /// *Custom…*: From / To and *Show Sep 1 – Sep 30*.
  fn custom_popup(&self, ctx: &mut Ctx, props: &ToolbarProps) -> Option<Element> {
    if !self.custom.get() {
      return None;
    }
    let content = ctx.mount_keyed::<CustomRange>(
      "usage-custom-range",
      CustomRangeProps {
        controls: props.controls.clone(),
        open: self.custom.clone(),
        range: props.range,
      },
    );
    Some(
      Popup::new(self.custom_anchor.clone(), content)
        .open(self.custom.clone())
        .placement(Placement::BottomEnd)
        .outside_press(OutsidePress::PassThrough)
        .into(),
    )
  }
}

/// *Sep 24 – Sep 30 · compared with Sep 17 – Sep 23*, with the number of projects across them.
fn readback(ctx: &Ctx, props: &ToolbarProps) -> Element {
  let range = format::range(ctx, props.range);
  let previous = format::range(ctx, props.range.previous());
  let mut text = match (&props.scope, &props.dashboard) {
    (Scope::All, Some(dashboard)) => ctx.t_args(
      "usage.readback.projects",
      [
        ("range", &*range),
        ("previous", &*previous),
        ("count", dashboard.projects.len().to_string().as_str()),
      ],
    ),
    _ => ctx.t_args(
      "usage.readback.compared",
      [("range", &*range), ("previous", &*previous)],
    ),
  };
  if let Some(dashboard) = props
    .dashboard
    .as_ref()
    .filter(|dashboard| !dashboard.unread.is_empty())
  {
    text = ctx.t_args(
      "usage.readback.unread",
      [
        ("readback", &*text),
        ("names", dashboard.unread.join(&ctx.t("usage.listSeparator")).as_str()),
      ],
    );
  }
  Text::new(&text)
    .id("usage-readback")
    .variant(TypographyStyle::Caption)
    .nowrap()
    .into()
}
