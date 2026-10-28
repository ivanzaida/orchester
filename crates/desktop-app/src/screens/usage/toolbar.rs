//! The pinned toolbar (`Toolbar · pinned` `W7V5GX`): the title *Usage* and what the page shows; the period
//! (*Today* · *7 days* · *30 days* · *Custom…*, `Period control` `xHbPG`) and *Group by* (*By hour* offered
//! for ranges up to 7 days); under them the filters and the period read back with the one it is compared
//! with, which takes a line of its own while filters are chosen (`YxAed`). Every project's page has no title:
//! its filters, *Project* first, have the period at their end and the read back under them (`rHA8G`).

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
  theme::{ExtraSpacing, ExtraTypography, UsageSpacing, UsageText},
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
  /// The project's name; `None` on every project's page.
  pub project: Option<Arc<str>>,
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
  /// Each menu filter's menu, in [`Facet::EVERY_PROJECT_MENUS`] order, and its button.
  filters: [(Signal<bool>, ElementRef); 6],
  group: (Signal<bool>, ElementRef),
  /// *Custom…*'s range: whether it is open, its segment and the period control, which places it.
  custom: (Signal<bool>, ElementRef, ElementRef),
  /// What a filter's menu searches for.
  search: Signal<String>,
}

impl Component for UsageToolbar {
  type Props = ToolbarProps;

  fn create(ctx: &mut Ctx) -> Self {
    let mut pair = || (ctx.signal(false), ElementRef::default());
    let toolbar = Self {
      filters: [pair(), pair(), pair(), pair(), pair(), pair()],
      group: pair(),
      custom: (ctx.signal(false), ElementRef::default(), ElementRef::default()),
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
    let controls = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(self.period(ctx, &props))
      .child(self.group_by(ctx, &props));
    let chips = dashboard.map_or_else(Vec::new, |dashboard| filters::chips(ctx, dashboard, &props.requery));
    let readback = Text::new(&readback(ctx, &props))
      .id("usage-readback")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap();
    let buttons: Vec<FilterButton> = self
      .menus(&props)
      .map(|(facet, (open, anchor))| {
        let (label, active) = filters::button_label(ctx, dashboard, facet);
        FilterButton::new(filters::facet_id(facet), label, open.clone(), anchor.clone()).active(active)
      })
      .collect();
    let filter_line = Row::new()
      .id("usage-filters")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(buttons)
      .with_children(chips.iter().cloned());
    let toolbar = Column::new()
      .id("usage-toolbar")
      .width(Dimension::Pct(100.0))
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
    let toolbar = match &props.project {
      Some(project) => {
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
        // The Delivery view beside Usage (ORC-50, no frame yet), switched to before the period.
        let views = props
          .requery
          .project()
          .map(|id| crate::screens::delivery::view_switch(ctx, id, false));
        let controls = Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .with_children(views)
          .child(controls);
        toolbar
          .padding(
            Padding::new()
              .top(UsageSpacing::Top.role())
              .bottom(ExtraSpacing::Inset.role())
              .left(UsageSpacing::Page.role())
              .right(UsageSpacing::Page.role()),
          )
          .spacing(SpacingSize::Lg)
          .child(title(ctx, project, controls))
          .child(filters)
      }
      // Every project's page: the filters with the period at their end, the period read back under them
      // (`Toolbar · pinned` `GeTGg`: 16 over, 14 under, 40 at the sides, 12 between).
      None => toolbar
        .padding(
          Padding::new()
            .top(SpacingSize::Lg)
            .bottom(ExtraSpacing::Inset.role())
            .left(UsageSpacing::Page.role())
            .right(UsageSpacing::Page.role()),
        )
        .spacing(SpacingSize::Md)
        .child(filter_line.child(Spacer::new().flex(1.0)).child(controls))
        .child(readback),
    };
    toolbar.with_children(self.popups(ctx, &props))
  }
}

/// *Usage*, what the page shows of `project`, and the period's `controls` at the end.
fn title(ctx: &Ctx, project: &str, controls: Row) -> Row {
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
      Text::new(&ctx.t_args("usage.description", [("project", project)]))
        .id("usage-description")
        .variant(UsageText::Description.role()),
    );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(heading)
    .child(controls)
}

impl UsageToolbar {
  /// The filters this page has a menu for, each with its menu's state and button: *Project* only on every
  /// project's page.
  fn menus<'a>(&'a self, props: &ToolbarProps) -> impl Iterator<Item = (Facet, &'a (Signal<bool>, ElementRef))> {
    let everywhere = props.project.is_none();
    Facet::EVERY_PROJECT_MENUS
      .into_iter()
      .zip(&self.filters)
      .filter(move |(facet, _)| everywhere || *facet != Facet::Project)
  }

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
      )
      .element(self.custom.1.clone()),
    ];
    Row::new()
      .ref_element(self.custom.2.clone())
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

  /// The menus and popovers: a filter's menu, *Group by*'s, and the custom range. Each is declared while closed
  /// too, so that a press on its button switches to it from another open dropdown.
  fn popups(&self, ctx: &mut Ctx, props: &ToolbarProps) -> Vec<Element> {
    let mut popups = Vec::new();
    for (facet, (open, anchor)) in self.menus(props) {
      let panel = if open.get() {
        filters::menu(ctx, props.dashboard.as_deref(), facet, &self.search, &props.requery)
      } else {
        Element::new()
      };
      let popup = menu(anchor.clone(), open.clone(), panel);
      // The Provider menu opens 12 under its button, as the Account menu did (`EU0ex`).
      let popup = if facet == Facet::Provider {
        popup.offset(0.0, ctx.theme().spacing_value(SpacingSize::Md).to_px())
      } else {
        popup
      };
      popups.push(popup.into());
    }
    let panel = if self.group.0.get() {
      self.group_menu(ctx, props)
    } else {
      Element::new()
    };
    popups.push(menu(self.group.1.clone(), self.group.0.clone(), panel).into());
    popups.push(custom_range::popover(ctx, props, &self.custom));
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

/// *Sep 24 – Sep 30 · compared with Sep 17 – Sep 23*, or *Today, Sep 30 until 14:20*; on every project's
/// page with how many projects were read (*· 4 projects*) and which could not be.
fn readback(ctx: &Ctx, props: &ToolbarProps) -> Arc<str> {
  let period = period_readback(ctx, props);
  let (None, Some(dashboard)) = (&props.project, &props.dashboard) else {
    return period;
  };
  let read = if dashboard.projects == 1 {
    ctx.t_args("usage.readback.projectsOne", [("readback", &*period)])
  } else {
    ctx.t_args(
      "usage.readback.projects",
      [
        ("readback", &*period),
        ("count", dashboard.projects.to_string().as_str()),
      ],
    )
  };
  if dashboard.unread.is_empty() {
    return read;
  }
  // A project that could not be read is named, never left out unsaid.
  let names = dashboard.unread.join(&ctx.t("usage.listSeparator"));
  ctx.t_args(
    "usage.readback.unread",
    [("readback", &*read), ("projects", names.as_str())],
  )
}

fn period_readback(ctx: &Ctx, props: &ToolbarProps) -> Arc<str> {
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
