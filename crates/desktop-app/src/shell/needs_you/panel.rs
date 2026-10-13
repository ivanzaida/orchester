//! The Needs you panel the title bar's pill opens (`Needs you panel` `BrP6i` of `Needs you · panel open · dark`
//! `VnLOT`, its states in `R1AZc` and answering in place in `zwuZt`): a popover anchored to the pill, not a
//! modal; Esc or a press outside closes it. Its head (`mxdKm`) says how many wait and in which projects; its
//! list (`RZz9r`) groups what waits by project, in the order of the tabs, oldest first in each, and scrolls
//! once the panel reaches the window's height; its footer (`mG6gL`) opens the active project's Overview.
//!
//! What waits is `tasks::needs_you`'s, so the panel lists exactly what every Needs you counts. Approvals and
//! questions with suggested answers expand in place around the conversation's own card (`in_place`), one at a
//! time; the others open their task: a plan, a decision after retries and an interrupted run its Summary, an
//! agent's question its Conversation, where the composer answers it. An item expanded here keeps its place
//! once it no longer waits, quiet, until the panel closes, so nothing moves under the pointer.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  needs_you::{WaitingItem, WaitingKind},
};
use parking_lot::Mutex;

use super::{
  Waiting,
  in_place::{InPlaceCard, InPlaceProps},
  item, waiting, words,
};
use crate::{
  navigation::Navigation,
  project::Projects,
  routes::{OverviewTab, ProjectPage, Route, TaskView},
  screens::task::cards::Decisions,
  theme::{ExtraSpacing, ExtraTypography, IconSize, NeedsYouShadow, NeedsYouSpacing, NeedsYouText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The panel's width (`BrP6i`).
pub(super) const PANEL_WIDTH: f32 = 460.0;
/// What the panel leaves of the window's height: 40 over it (the title bar and the gap under the pill) and 32
/// under it, above the status bar (`BrP6i` in `VnLOT`, 828 high in a 900 window).
const WINDOW_ROOM: f32 = 72.0;
/// How often the waiting times move on while the panel shows.
const TICK: Duration = Duration::from_secs(30);

/// The panel's open state, which the pill holds.
#[derive(Clone)]
pub(super) struct PanelProps(pub(super) Signal<bool>);

impl PartialEq for PanelProps {
  fn eq(&self, other: &Self) -> bool {
    self.0.id() == other.0.id()
  }
}

impl DevtoolsInspectable for PanelProps {}

/// An item of a project, as the list shows it: whether it still waits.
#[derive(Clone)]
struct Shown {
  project: ProjectId,
  key: Arc<str>,
  item: WaitingItem,
  waits: bool,
}

/// What the list shows of one project.
struct Group {
  waiting: Waiting,
  items: Vec<Shown>,
}

pub(super) struct NeedsYouPanel {
  decisions: Decisions,
  /// The item expanded in place, by project and key.
  expanded: Signal<Option<(ProjectId, Arc<str>)>>,
  /// Items expanded since the panel opened, which keep their place once they no longer wait.
  kept: Mutex<Vec<Shown>>,
  tick: Signal<u64>,
  interval: Interval,
}

impl Component for NeedsYouPanel {
  type Props = PanelProps;

  fn create(ctx: &mut Ctx) -> Self {
    let decisions = Decisions::new(ctx);
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self {
      decisions,
      expanded: ctx.signal(None),
      kept: Mutex::new(Vec::new()),
      tick,
      interval,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First, as the panel made it: lurq keeps a component's futures by the order it asks for them.
    self.decisions.keep(ctx);
    ctx.provide(self.decisions.clone());
    self.tick.get();
    self.interval.start();
    let open = ctx.props::<PanelProps>().0.clone();
    let navigation = Navigation::of(ctx);
    let projects = Projects::of(ctx);
    let tabs = navigation.tabs();
    let active = tabs.active().route().project();
    let groups = self.groups(&projects, waiting(&projects, &tabs));
    let total: usize = groups.iter().map(|group| group.waiting.count).sum();
    let names: Vec<Arc<str>> = groups
      .iter()
      .map(|group| Arc::from(group.waiting.name.as_str()))
      .collect();
    let height = ctx.window().info().resolved_height - WINDOW_ROOM;
    let list = if groups.iter().all(|group| group.items.is_empty()) {
      empty(ctx)
    } else {
      self.list(ctx, &groups, active, (&navigation, &open))
    };
    Column::new()
      .id("needs-you-panel")
      .describe("count", total.to_string())
      .width(Dimension::Px(PANEL_WIDTH))
      .max_height(Dimension::Px(height))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Lg)
      .box_shadow(NeedsYouShadow::Panel.role())
      .clip()
      .child(head(ctx, total, &words::sub(ctx, &names, total > 0), &open))
      .child(list)
      .child(footer(ctx, &groups, active, (&navigation, &open)))
  }
}

impl NeedsYouPanel {
  /// What waits in each project the tabs show, with the items expanded here that no longer wait kept in their
  /// place.
  fn groups(&self, projects: &Projects, all: Vec<Waiting>) -> Vec<Group> {
    let kept = self.kept.lock();
    all
      .into_iter()
      .map(|waiting| {
        let project = waiting.project;
        let mut items: Vec<Shown> = projects
          .feed(project)
          .tasks
          .with(|tasks| tasks.ready().map(ProjectTasks::waiting))
          .unwrap_or_default()
          .into_iter()
          .map(|item| Shown {
            project,
            key: key_of(&item),
            item,
            waits: true,
          })
          .collect();
        for gone in kept.iter().filter(|kept| kept.project == project) {
          if !items.iter().any(|shown| shown.key == gone.key) {
            items.push(Shown {
              waits: false,
              ..gone.clone()
            });
          }
        }
        items.sort_by_key(|shown| shown.item.since);
        Group { waiting, items }
      })
      .collect()
  }

  fn list(
    &self,
    ctx: &mut Ctx,
    groups: &[Group],
    active: Option<ProjectId>,
    (navigation, open): (&Navigation, &Signal<bool>),
  ) -> Element {
    let mut rows: Vec<Element> = Vec::new();
    for group in groups.iter().filter(|group| !group.items.is_empty()) {
      let project = group.waiting.project;
      let this_tab = active == Some(project);
      rows.push(item::group(
        ctx,
        &format!("needs-you-group-{project}"),
        (group.waiting.name.as_str(), this_tab),
        group.waiting.count,
      ));
      let waiting: Arc<[Arc<str>]> = group
        .items
        .iter()
        .filter(|shown| shown.waits)
        .map(|shown| shown.key.clone())
        .collect();
      for shown in &group.items {
        rows.push(self.row(ctx, shown, &waiting, (navigation, open)));
      }
    }
    ScrollVertical::new(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(SpacingSize::Xs)
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm)
            .bottom(SpacingSize::Sm),
        )
        .spacing(ExtraSpacing::Tight.role())
        .with_children(rows),
    )
    .id("needs-you-list")
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
    .min_height(Dimension::Px(0.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .into()
  }

  /// One item, collapsed or expanded around the card that answers it.
  fn row(
    &self,
    ctx: &mut Ctx,
    shown: &Shown,
    waiting: &Arc<[Arc<str>]>,
    (navigation, open): (&Navigation, &Signal<bool>),
  ) -> Element {
    let id = format!("needs-you-item-{}", shown.key);
    let item = &shown.item;
    let at = (shown.project, shown.key.clone());
    let expanded = self.expanded.get().as_ref() == Some(&at);
    let in_place = item.kind.card_key().zip(item.task.as_ref()).zip(item.run);
    let Some(((key, task), _)) = in_place else {
      let route = route_of(shown);
      let press = go(navigation, open, route);
      let glyph = if matches!(item.kind, WaitingKind::Question(_)) {
        Icon::MessageCircle
      } else {
        Icon::ArrowUpRight
      };
      return item::collapsed(ctx, &id, item, (glyph, press));
    };
    if !expanded {
      let (signal, at) = (self.expanded.clone(), at);
      let press: Arc<dyn Fn() + Send + Sync> = Arc::new(move || signal.set(Some(at.clone())));
      return item::collapsed(ctx, &id, item, (Icon::ChevronDown, press));
    }
    {
      let mut kept = self.kept.lock();
      if !kept
        .iter()
        .any(|kept| kept.project == shown.project && kept.key == shown.key)
      {
        kept.push(shown.clone());
      }
    }
    let card = ctx.mount_keyed::<InPlaceCard>(
      &format!("{id}-card"),
      InPlaceProps {
        id: id.clone(),
        project: shown.project,
        task: task.id,
        key,
        step: item.step.as_ref().map(|step| (step.number, step.name.clone())),
        waiting: waiting.clone(),
      },
    );
    let collapse: Arc<dyn Fn() + Send + Sync> = {
      let signal = self.expanded.clone();
      Arc::new(move || signal.set(None))
    };
    let conversation = go(
      navigation,
      open,
      task_route(shown.project, &task.key, TaskView::Conversation),
    );
    item::expanded(ctx, &id, (item, shown.waits), (collapse, conversation), card)
  }
}

/// How an item is told from the others while the panel shows: its card's key when it is answered in place,
/// else its task's.
fn key_of(item: &WaitingItem) -> Arc<str> {
  item.kind.card_key().unwrap_or_else(|| {
    let task = item.task.as_ref().map_or_else(String::new, |task| task.id.to_string());
    Arc::from(format!("task-{task}"))
  })
}

/// Where an item that is not answered in place leads: an agent's question to the task's Conversation, where
/// the composer answers it; the others to its Summary.
fn route_of(shown: &Shown) -> Option<Route> {
  let task = shown.item.task.as_ref()?;
  let view = match shown.item.kind {
    WaitingKind::Question(_) => TaskView::Conversation,
    _ => TaskView::of(crate::routes::TaskTab::Summary),
  };
  task_route(shown.project, &task.key, view)
}

fn task_route(project: ProjectId, key: &str, view: TaskView) -> Option<Route> {
  let key = key.parse().ok()?;
  Some(Route::Project(project, ProjectPage::Task(key, view)))
}

/// Goes to `route` and closes the panel, which the page opening takes the place of.
fn go(navigation: &Navigation, open: &Signal<bool>, route: Option<Route>) -> Arc<dyn Fn() + Send + Sync> {
  let (navigation, open) = (navigation.clone(), open.clone());
  Arc::new(move || {
    if let Some(route) = route.clone() {
      navigation.go(route);
    }
    open.set(false);
  })
}

/// The head: the hand, *Needs you* and how many wait, the close, and the projects under them.
fn head(ctx: &mut Ctx, total: usize, sub: &str, open: &Signal<bool>) -> Element {
  let color = if total > 0 {
    PaletteColor::Warning
  } else {
    PaletteColor::TextMuted
  };
  let close = {
    let open = open.clone();
    IconButton::new("needs-you-close", Icon::X, ctx.t("needs_you.close"))
      .inspector()
      .on_click(move || open.set(false))
      .render(ctx)
  };
  Column::new()
    .id("needs-you-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(crate::theme::ExtraSpacing::Loose.role())
        .bottom(crate::theme::ExtraSpacing::Loose.role())
        .right(SpacingSize::Sm)
        .left(SpacingSize::Lg),
    )
    .spacing(crate::theme::ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(NeedsYouSpacing::TitleRow.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Hand, IconSize::Nav, color.clone()))
        .child(
          Text::new(&ctx.t("needs_you.title"))
            .id("needs-you-title")
            .variant(NeedsYouText::Title.role()),
        )
        .child(
          Text::new(&total.to_string())
            .id("needs-you-count")
            .variant(NeedsYouText::Count.role())
            .color(color),
        )
        .child(Spacer::new().flex(1.0))
        .child(close),
    )
    .child(Text::new(sub).id("needs-you-sub").variant(ExtraTypography::Note.role()))
    .into()
}

/// Nothing waits (`oMseR`): the hand, *Nothing needs you* and what appears here.
fn empty(ctx: &Ctx) -> Element {
  Column::new()
    .id("needs-you-empty")
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Sm),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(
          NeedsYouSpacing::EmptySides.role(),
          crate::theme::ExtraSpacing::Roomy.role(),
        ))
        .spacing(crate::theme::ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Hand, IconSize::Xl, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("needs_you.empty.title"))
            .id("needs-you-empty-title")
            .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(&ctx.t("needs_you.empty.text"))
            .id("needs-you-empty-text")
            .variant(NeedsYouText::Empty.role())
            .text_align(TextAlign::Center)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// *Show all in Kontur's Overview*, of the active tab's project or the first listed, and *Esc closes*.
fn footer(
  ctx: &Ctx,
  groups: &[Group],
  active: Option<ProjectId>,
  (navigation, open): (&Navigation, &Signal<bool>),
) -> Element {
  let chosen = groups
    .iter()
    .find(|group| Some(group.waiting.project) == active)
    .or_else(|| groups.first())
    .map(|group| &group.waiting);
  let show_all = chosen.map(|waiting| {
    let route = Route::Project(waiting.project, ProjectPage::Overview(OverviewTab::NeedsYou));
    let press = go(navigation, open, Some(route));
    ActionButton::new(
      "needs-you-show-all",
      ButtonKind::Ghost,
      ctx.t_args("needs_you.showAll", [("project", waiting.name.as_str())]),
    )
    .icon(Icon::LayoutDashboard)
    .on_click(move || press())
  });
  Row::new()
    .id("needs-you-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(crate::theme::ExtraSpacing::Snug.role())
        .bottom(crate::theme::ExtraSpacing::Snug.role())
        .right(SpacingSize::Md)
        .left(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(show_all)
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t("needs_you.escCloses"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}
