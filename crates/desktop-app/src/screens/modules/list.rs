//! The Modules list (`01 · Modules` `m63AnV`, `04 · Modules · none yet` `jztMU`, the Closed line in `07` `qu3oT`): the
//! Modules header with *Search modules* (`/` reaches it) and New module, what modules are on the status line, and the
//! views Open and Closed with their counts as the tabs, each a place of its own (`?tab=closed`). Under them a row per
//! module of the view that the search finds, oldest first ([`super::list_row`]), each with its forecast once the
//! project's delivery is read off the UI thread. A project without modules shows the empty state in place of the rows
//! and no search; a view without modules says so in one line.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ModuleId, module::ModuleState, time::Timestamp};
use orchester_workspace::{
  background,
  delivery::Forecast,
  messages::Message,
  modules::data::{ModuleSummary, forecasts, summaries},
};

use super::{header, list_row};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ModuleListTab, ProjectPage, Route},
  theme::{ExtraSpacing, TaskSpacing, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    message::message_text,
    skeleton::{Skeleton, fading},
    state::{EmptyState, load_error},
    tabs::{PageTab, page_tabs},
  },
};

/// The search's width (`Search` `cm8xD`).
const SEARCH_WIDTH: f32 = 240.0;
/// The empty state's column, which its explanation fills (`Column` `yJdJ9`).
const EMPTY_MEASURE: f32 = 420.0;

#[derive(Clone, PartialEq)]
pub struct ListProps {
  pub project: ShownProject,
  pub tab: ModuleListTab,
}

impl DevtoolsInspectable for ListProps {}

/// Each module's forecast, as read.
#[derive(Clone)]
pub struct Forecasts(pub Arc<HashMap<ModuleId, Forecast>>);

impl PartialEq for Forecasts {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Forecasts {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForecastsFailed(pub Message);

impl DevtoolsInspectable for ForecastsFailed {}

pub struct ModulesList {
  search: Signal<String>,
  search_input: ElementRef,
  focus_search: Signal<bool>,
}

impl Component for ModulesList {
  type Props = ListProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      search: ctx.signal(String::new()),
      search_input: ElementRef::default(),
      focus_search: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ListProps { project, tab } = ctx.props::<ListProps>().clone();
    if self.focus_search.get() {
      ctx.focus(&self.search_input);
      self.focus_search.set(false);
    }
    let state = project.feed.tasks.get();
    let listed = match &state {
      TasksState::Ready(tasks) => Some(summaries(&tasks.modules, &tasks.tasks)),
      TasksState::Loading | TasksState::Failed(_) => None,
    };
    let revision = match &state {
      TasksState::Ready(tasks) => Arc::as_ptr(&tasks.tasks) as usize,
      TasksState::Loading | TasksState::Failed(_) => 0,
    };
    let forecasts = read_forecasts(ctx, project.open.repo_root.clone(), revision);
    let header = self.header(ctx, &project, tab, listed.as_deref());
    let body = match (&state, &listed) {
      (TasksState::Ready(_), Some(listed)) => self.body(ctx, (&project, tab), listed, forecasts.as_ref()),
      (TasksState::Failed(error), _) => {
        let feed = project.feed.clone();
        let words = message_text(ctx, error);
        let failed = load_error(
          ctx,
          "modules-load-error",
          (&ctx.t("modules.readFailed"), &words),
          move || feed.saved(),
        );
        padded(failed)
      }
      _ => reading(ctx),
    };
    Column::new()
      .id("screen-modules")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .on_key_down(slash(self.focus_search.clone()))
      .child(header)
      .child(body)
  }
}

impl ModulesList {
  fn header(
    &self,
    ctx: &mut Ctx,
    project: &ShownProject,
    tab: ModuleListTab,
    listed: Option<&[ModuleSummary]>,
  ) -> Element {
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let new = ActionButton::new("modules-new", ButtonKind::Primary, ctx.t("modules.newModule"))
      .icon(Icon::Plus)
      .opens(navigation.to(Route::Project(id, ProjectPage::NewModule)));
    // While the modules are read the search stands where it will (`05 · Modules · loading` `beo7S`).
    let reading = matches!(project.feed.tasks.get(), TasksState::Loading);
    let any = reading || listed.is_some_and(|listed| !listed.is_empty());
    let search = any.then(|| {
      FieldInput::new("modules-search", ctx.t("modules.search.label"), self.search.clone())
        .search()
        .keys(ctx.t("modules.search.keys"))
        .clearable(ctx.t("modules.search.clear"))
        .placeholder(ctx.t("modules.search.placeholder"))
        .width(Dimension::Px(SEARCH_WIDTH))
        .element(self.search_input.clone())
        .render(ctx)
    });
    let status = Text::new(&ctx.t("modules.description"))
      .id("modules-description")
      .variant(TypographyStyle::Description)
      .color(lurq::app::theme::PaletteColor::TextMuted)
      .nowrap();
    header::header(header::Header {
      id: "modules-header",
      back: None,
      title: header::title(ctx, "modules-title", &ctx.t("modules.title")),
      actions: search.into_iter().chain([new.into()]).collect(),
      status: header::status_line("modules-status", vec![status.into()]),
      tabs: tabs(ctx, id, (tab, reading), listed),
    })
  }

  /// The rows of the view the search finds, or why there are none.
  fn body(
    &self,
    ctx: &mut Ctx,
    (project, tab): (&ShownProject, ModuleListTab),
    listed: &[ModuleSummary],
    forecasts: Option<&Forecasts>,
  ) -> Element {
    if listed.is_empty() {
      return empty(ctx, project);
    }
    let in_view: Vec<&ModuleSummary> = listed.iter().filter(|summary| shows(tab, summary)).collect();
    let search = self.search.get();
    let found: Vec<&ModuleSummary> = in_view
      .iter()
      .copied()
      .filter(|summary| finds(&search, summary))
      .collect();
    let id = project.open.project.id();
    let content: Vec<Element> = if in_view.is_empty() {
      let key = match tab {
        ModuleListTab::Open => "modules.openEmpty",
        ModuleListTab::Closed => "modules.closedEmpty",
      };
      vec![line("modules-view-empty", &ctx.t(key))]
    } else if found.is_empty() {
      let words = ctx.t_args("modules.search.noMatch", [("query", search.trim())]);
      vec![line("modules-no-match", &words)]
    } else {
      found
        .iter()
        .enumerate()
        .map(|(index, summary)| {
          let forecast = forecasts.and_then(|forecasts| forecasts.0.get(&summary.module.id()));
          list_row::row(ctx, (id, index), summary, forecast)
        })
        .collect()
    };
    let rows = Column::new()
      .id("modules-list")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .bottom(TaskSpacing::Foot.role())
          .left(TaskSpacing::Side.role())
          .right(TaskSpacing::Side.role()),
      )
      .spacing(ExtraSpacing::Tight.role())
      .with_children(content);
    ScrollVertical::new(rows).id("modules-scroll").flex(1.0).into()
  }
}

/// Open and Closed with their counts, 12 `text_muted` after the label (`MYsfj`), once the modules are read; each count
/// a bone while they are `reading`.
fn tabs(
  ctx: &mut Ctx,
  project: orchester_core::id::ProjectId,
  (chosen, reading): (ModuleListTab, bool),
  listed: Option<&[ModuleSummary]>,
) -> Element {
  let navigation = Navigation::of(ctx);
  let tabs: Vec<PageTab> = ModuleListTab::ALL
    .into_iter()
    .map(|tab| {
      let word = ctx.t(match tab {
        ModuleListTab::Open => "modules.state.open",
        ModuleListTab::Closed => "modules.state.closed",
      });
      let open = navigation.to(Route::Project(project, ProjectPage::Modules(tab)));
      let page_tab = PageTab::to(format!("modules-tab-{}", tab.name()), word, tab == chosen, open);
      match listed.filter(|listed| !listed.is_empty()) {
        Some(listed) => page_tab.count(listed.iter().filter(|summary| shows(tab, summary)).count(), false),
        None if reading => page_tab.count_reading(),
        None => page_tab,
      }
    })
    .collect();
  page_tabs(ctx, "modules-tabs", tabs).into()
}

/// Each module's row bones while the modules are read (`05 · Modules · loading` `beo7S`): its name, description and
/// forecast.
const READING: [(f32, f32, f32); 6] = [
  (104.0, 218.0, 88.0),
  (120.0, 186.0, 164.0),
  (98.0, 164.0, 120.0),
  (130.0, 228.0, 98.0),
  (108.0, 196.0, 130.0),
  (88.0, 174.0, 108.0),
];

/// The rows while the modules are read, in the list's own room, the lower rows fading. *Reading the modules…*
/// (`modules.reading`) names them.
fn reading(ctx: &mut Ctx) -> Element {
  let rows = READING.iter().map(|words| list_row::bones(ctx, *words)).collect();
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .bottom(TaskSpacing::Foot.role())
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role()),
    )
    .spacing(ExtraSpacing::Tight.role())
    .with_children(fading(rows));
  Skeleton::new("modules-loading", ctx.t("modules.reading")).show(ctx, rows)
}

fn shows(tab: ModuleListTab, summary: &ModuleSummary) -> bool {
  let state = summary.module.details().state();
  matches!(
    (tab, state),
    (ModuleListTab::Open, ModuleState::Open) | (ModuleListTab::Closed, ModuleState::Closed)
  )
}

/// Whether `search` is in the module's name or description, whatever their case.
fn finds(search: &str, summary: &ModuleSummary) -> bool {
  let search = search.trim().to_lowercase();
  let details = summary.module.details();
  search.is_empty()
    || details.name().as_str().to_lowercase().contains(&search)
    || details.description().to_lowercase().contains(&search)
}

/// One quiet line in place of the rows (`Line` `s9YFIU`): 12 `text_secondary`, centred, 16 over and under.
fn line(id: &str, words: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Lg))
    .justify(Justify::Center)
    .child(Text::new(words).id(id.to_owned()).variant(TypographyStyle::Description))
    .into()
}

/// A project without modules (`04 · Modules · none yet` `jztMU`): the empty state with New module, lifted over the
/// middle of the panel.
fn empty(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  let new = ActionButton::new("modules-empty-new", ButtonKind::Primary, ctx.t("modules.newModule"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::Project(project.open.project.id(), ProjectPage::NewModule)));
  let state = EmptyState::new(
    "modules-empty",
    Icon::Target,
    ctx.t("modules.empty.title"),
    ctx.t("modules.empty.body"),
  )
  .unframed()
  .centered()
  .gap(SpacingSize::Lg)
  .measure(EMPTY_MEASURE)
  .action(new);
  Column::new()
    .id("modules-empty-page")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::new().bottom(TasksSpacing::EmptyLift.role()))
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .child(state)
    .into()
}

/// The body's own room under the header, for the reading and failure lines.
fn padded(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role()),
    )
    .child(content)
    .into()
}

/// `/` puts the keys in the search, unless a field has them.
fn slash(focus: Signal<bool>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  move |event: KeyboardEvent| {
    if event.default_prevented() || event.text_input_focused || event.ctrl || event.alt || event.meta {
      return;
    }
    if event.key == "/" {
      event.prevent_default();
      focus.set(true);
    }
  }
}

/// Every module's forecast, read off the UI thread again whenever the project's tasks are; a failed read leaves the
/// rows without one and is logged.
fn read_forecasts(ctx: &mut Ctx, root: PathBuf, revision: usize) -> Option<Forecasts> {
  let state = ctx
    .future((root, revision), |(root, _)| async move {
      background::run(move || {
        let db = orchester_store::ProjectDb::open(&root)?;
        forecasts(&db, Timestamp::now())
      })
      .await
      .map_err(ForecastsFailed)?
      .map(|read| Forecasts(Arc::new(read)))
      .map_err(|error| {
        tracing::warn!(%error, "cannot forecast the modules");
        ForecastsFailed(orchester_workspace::messages::store_error(&error))
      })
    })
    .state()
    .get();
  state.data
}
