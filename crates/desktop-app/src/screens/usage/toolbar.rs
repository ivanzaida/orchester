//! The pinned toolbar (`Toolbar · pinned` `W7V5GX`): the title *Usage* and what the page shows; the period
//! (*Today* · *7 days* · *30 days* · *Custom…*, `Period control` `xHbPG`) and *Group by* (*By hour* offered
//! for ranges up to 7 days); under them the filters and the period read back with the one it is compared
//! with, which takes a line of its own while filters are chosen (`YxAed`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  time::Timestamp,
  usage::{GroupBy, Grouping, Period, local_date},
};
use orchester_workspace::{
  display,
  usage::{Dashboard, Facet, UsageQuery},
};

use super::{Requery, custom_range, filters, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, UsageSpacing},
  ui::{
    filter::{FilterButton, filter_menu_panel},
    icon::Icon,
    menu::{MenuItem, menu},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The *Group by* menu's width (`Group by menu` `GdEHA`).
const GROUP_MENU_WIDTH: f32 = 240.0;

#[derive(Clone)]
pub struct ToolbarProps {
  pub project: Arc<str>,
  pub query: UsageQuery,
  pub dashboard: Option<Arc<Dashboard>>,
  pub requery: Requery,
}

impl PartialEq for ToolbarProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.query == other.query
      && match (&self.dashboard, &other.dashboard) {
        (Some(one), Some(other)) => Arc::ptr_eq(one, other),
        (one, other) => one.is_none() && other.is_none(),
      }
  }
}

impl DevtoolsInspectable for ToolbarProps {}

pub struct UsageToolbar {
  /// Each menu filter's menu, in [`Facet::MENUS`] order, and its button.
  filters: [(Signal<bool>, ElementRef); 5],
  group: (Signal<bool>, ElementRef),
  custom: (Signal<bool>, ElementRef),
  /// What a filter's menu searches for.
  search: Signal<String>,
}

impl Component for UsageToolbar {
  type Props = ToolbarProps;

  fn create(ctx: &mut Ctx) -> Self {
    let mut pair = || (ctx.signal(false), ElementRef::default());
    let toolbar = Self {
      filters: [pair(), pair(), pair(), pair(), pair()],
      group: pair(),
      custom: pair(),
      search: ctx.signal(String::new()),
    };
    // A menu opens on its whole list: what the last one searched for is forgotten.
    for (open, _) in &toolbar.filters {
      let search = toolbar.search.clone();
      ctx.watch(open, move |open: &bool| {
        if *open {
          search.set(String::new());
        }
      });
    }
    toolbar
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolbarProps>().clone();
    let dashboard = props.dashboard.as_deref();
    let heading = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Text::new(&ctx.t("usage.title"))
          .variant(ExtraTypography::Display.role())
          .nowrap(),
      )
      .child(
        Text::new(&ctx.t_args("usage.description", [("project", &*props.project)]))
          .id("usage-description")
          .variant(TypographyStyle::Body)
          .color(PaletteColor::TextSecondary),
      );
    let controls = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(self.period(ctx, &props))
      .child(self.group_by(ctx, &props));
    let title = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::End)
      .child(heading)
      .child(controls);
    let chips = dashboard.map_or_else(Vec::new, |dashboard| filters::chips(ctx, dashboard, &props.requery));
    let readback = Text::new(&readback(ctx, &props))
      .id("usage-readback")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap();
    let buttons = Facet::MENUS
      .into_iter()
      .zip(&self.filters)
      .map(|(facet, (open, anchor))| {
        let (label, active) = filters::button_label(ctx, dashboard, facet);
        FilterButton::new(filters::facet_id(facet), label, open.clone(), anchor.clone()).active(active)
      });
    let filter_line = Row::new()
      .id("usage-filters")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(buttons)
      .with_children(chips.iter().cloned());
    // Beside the filters, or 8 under them and their chips (`Filters` `N20RN`).
    let filters = if chips.is_empty() {
      Column::new().child(filter_line.child(Spacer::new().flex(1.0)).child(readback))
    } else {
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(filter_line)
        .child(readback)
    }
    .width(Dimension::Pct(100.0));
    Column::new()
      .id("usage-toolbar")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(UsageSpacing::Top.role())
          .bottom(ExtraSpacing::Inset.role())
          .left(UsageSpacing::Page.role())
          .right(UsageSpacing::Page.role()),
      )
      .spacing(SpacingSize::Lg)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(title)
      .child(filters)
      .with_children(self.popups(ctx, &props))
  }
}

impl UsageToolbar {
  /// *Today* · *7 days* · *30 days* · *Custom…*; *Custom…* opens the range's fields under the control,
  /// and reads as chosen while they are open.
  fn period(&self, ctx: &Ctx, props: &ToolbarProps) -> Element {
    let today = local_date(Timestamp::now());
    let picking = self.custom.0.get();
    let chosen = props.query.period;
    let preset = |id: &str, key: &str, period: Period| {
      let (requery, query) = (props.requery.clone(), props.query.with_period(period, today));
      Segment::new(
        format!("usage-period-{id}"),
        SegmentFace::Text(ctx.t(key)),
        !picking && chosen == period,
        move || requery.show(query.clone()),
      )
    };
    let custom = self.custom.0.clone();
    let segments = [
      preset("today", "usage.period.today", Period::Today),
      preset("week", "usage.period.week", Period::Week),
      preset("month", "usage.period.month", Period::Month),
      Segment::new(
        "usage-period-custom",
        SegmentFace::Text(ctx.t("usage.period.custom")),
        picking || matches!(chosen, Period::Custom(_)),
        move || custom.update(|open| *open = !*open),
      ),
    ];
    Row::new()
      .ref_element(self.custom.1.clone())
      .child(segmented("usage-period", segments, Focus::Pointer))
      .into()
  }

  /// *Group by*: *By hour* or *By day*, as *Tokens over time* groups the range.
  fn group_by(&self, ctx: &Ctx, props: &ToolbarProps) -> Element {
    let range = props.query.range(local_date(Timestamp::now()));
    let label = ctx.t(match props.query.grouping(range) {
      Grouping::Hour => "usage.groupBy.hour",
      Grouping::Day => "usage.groupBy.day",
    });
    FilterButton::new("usage-group-by", label, self.group.0.clone(), self.group.1.clone()).into()
  }

  /// The open menus and popovers: a filter's menu, *Group by*'s, and the custom range.
  fn popups(&self, ctx: &mut Ctx, props: &ToolbarProps) -> Vec<Element> {
    let mut popups = Vec::new();
    for (facet, (open, anchor)) in Facet::MENUS.into_iter().zip(&self.filters) {
      if open.get() {
        let panel = filters::menu(ctx, props.dashboard.as_deref(), facet, &self.search, &props.requery);
        popups.push(menu(anchor.clone(), open.clone(), panel).into());
      }
    }
    if self.group.0.get() {
      let panel = self.group_menu(ctx, props);
      popups.push(menu(self.group.1.clone(), self.group.0.clone(), panel).into());
    }
    if self.custom.0.get() {
      popups.push(custom_range::popover(ctx, props, &self.custom));
    }
    popups
  }

  fn group_menu(&self, ctx: &mut Ctx, props: &ToolbarProps) -> Element {
    let today = local_date(Timestamp::now());
    let range = props.query.range(today);
    let chosen = props.query.grouping(range);
    let choose = |group: GroupBy| {
      let (requery, query) = (
        props.requery.clone(),
        UsageQuery {
          group,
          ..props.query.clone()
        },
      );
      move || requery.show(query.clone())
    };
    let hour = if range.offers_hours() {
      MenuItem::new("usage-group-hour", ctx.t("usage.groupBy.hour"), choose(GroupBy::Hour))
    } else {
      MenuItem::unavailable(
        "usage-group-hour",
        ctx.t("usage.groupBy.hour"),
        ctx.t("usage.groupBy.hourUnavailable"),
      )
      .without_lock()
    };
    let day = MenuItem::new("usage-group-day", ctx.t("usage.groupBy.day"), choose(GroupBy::Day));
    let open = self.group.0.clone();
    filter_menu_panel("usage-group-menu", GROUP_MENU_WIDTH)
      .child(
        hour
          .icon(Icon::Clock3)
          .chosen(chosen == Grouping::Hour)
          .render(ctx, &open),
      )
      .child(
        day
          .icon(Icon::CalendarDays)
          .chosen(chosen == Grouping::Day)
          .render(ctx, &open),
      )
      .into()
  }
}

/// *Sep 24 – Sep 30 · compared with Sep 17 – Sep 23*, or *Today, Sep 30 until 14:20*.
fn readback(ctx: &Ctx, props: &ToolbarProps) -> Arc<str> {
  let now = Timestamp::now();
  let today = local_date(now);
  let range = props.query.range(today);
  if range.days() == 1 && range.first() == today {
    return ctx.t_args(
      "usage.readback.today",
      [
        ("day", &*words::day(ctx, today)),
        ("time", display::clock(now).as_str()),
      ],
    );
  }
  ctx.t_args(
    "usage.readback.compared",
    [
      ("range", &*words::range(ctx, range)),
      ("previous", &*words::range(ctx, range.previous())),
    ],
  )
}
