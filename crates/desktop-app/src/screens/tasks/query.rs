//! What narrows and orders the list (`Query line 1` `nZ8QQ`): the search, the pipeline filter, the sort, and
//! that sorting never changes the run queue. The search is part of the list's path (`?q=`), taking the
//! current entry's place as it is typed, so Back leaves the list rather than replaying the typing; the
//! pipeline and the sort belong to the open list. The Status, Priority and Tags filters and Manage tags
//! (`Query line 2` `pW6TT`) are not built yet (D4).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  modules::ModuleFilter,
  tasks::{
    browse::{PipelineFilter, TaskSort},
    data::ProjectTasks,
  },
};

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{DISABLED_OPACITY, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::{FieldInput, SelectField},
    icon::{Icon, icon},
  },
};

/// `Search` `YDswQ`, `Pipeline filter` `hEEE0` and `Sort` `qkzdc`; the module filter (ORC-48, no frame yet) is as
/// wide as the pipeline filter.
const SEARCH_WIDTH: f32 = 300.0;
const PIPELINE_WIDTH: f32 = 180.0;
const SORT_WIDTH: f32 = 160.0;

/// What the person chose to narrow and order the list.
pub(super) struct Choices {
  pub search: Signal<String>,
  pub pipeline: Signal<PipelineFilter>,
  pub module: Signal<ModuleFilter>,
  pub sort: Signal<TaskSort>,
}

/// The module filter: every module, no module, or one of the project's modules once the tasks are read.
fn module_filter(ctx: &Ctx, choices: &Choices, tasks: Option<&ProjectTasks>) -> Element {
  let mut modules = vec![
    SelectOption::new(ModuleFilter::Any, ctx.t("tasks.query.allModules")),
    SelectOption::new(ModuleFilter::None, ctx.t("tasks.query.noModule")),
  ];
  modules.extend(
    tasks
      .into_iter()
      .flat_map(|tasks| tasks.modules.iter())
      .map(|module| SelectOption::new(ModuleFilter::In(module.id()), module.name().as_str())),
  );
  SelectField::new(
    "tasks-module-filter",
    ctx.t("tasks.query.module"),
    choices.module.clone(),
    modules,
  )
  .width(Dimension::Px(PIPELINE_WIDTH))
  .render(ctx)
}

/// The search, the pipeline filter with the project's published pipelines, the sort and the note on
/// sorting. `tasks` is `None` until they are read: the filter then offers all pipelines only.
pub(super) fn query(ctx: &mut Ctx, choices: &Choices, tasks: Option<&ProjectTasks>) -> Row {
  let search = FieldInput::new("tasks-search", ctx.t("tasks.query.search"), choices.search.clone())
    .search()
    .clearable(ctx.t("tasks.query.clearSearch"))
    .placeholder(ctx.t("tasks.query.searchPlaceholder"))
    .width(Dimension::Px(SEARCH_WIDTH))
    .render(ctx);
  let mut pipelines = vec![SelectOption::new(
    PipelineFilter(None),
    ctx.t("tasks.query.allPipelines"),
  )];
  pipelines.extend(
    tasks
      .into_iter()
      .flat_map(|tasks| tasks.pipelines.iter())
      .map(|pipeline| SelectOption::new(PipelineFilter(Some(pipeline.id)), pipeline.name.clone())),
  );
  let pipeline = SelectField::new(
    "tasks-pipeline-filter",
    ctx.t("tasks.query.pipeline"),
    choices.pipeline.clone(),
    pipelines,
  )
  .width(Dimension::Px(PIPELINE_WIDTH))
  .render(ctx);
  let sorts = [
    (TaskSort::Manual, "tasks.sort.manual"),
    (TaskSort::Newest, "tasks.sort.newest"),
    (TaskSort::Title, "tasks.sort.title"),
    (TaskSort::Status, "tasks.sort.status"),
    (TaskSort::Priority, "tasks.sort.priority"),
  ]
  .map(|(sort, key)| SelectOption::new(sort, ctx.t(key)))
  .to_vec();
  let sort = SelectField::new("tasks-sort", ctx.t("tasks.query.sort"), choices.sort.clone(), sorts)
    .width(Dimension::Px(SORT_WIDTH))
    .render(ctx);
  Row::new()
    .id("tasks-query")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(search)
    .child(pipeline)
    .child(module_filter(ctx, choices, tasks))
    .child(sort)
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id("tasks-sort-note")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("tasks.query.sortNote")).variant(ExtraTypography::Note.role())),
    )
}

/// The query laid out but faded and out of reach while the tasks cannot be read (`Query` in `oDyZB`).
pub(super) fn out_of_reach(query: Row) -> Element {
  query
    .opacity(DISABLED_OPACITY)
    .pointer_events_none()
    .describe("state", "unavailable")
    .into()
}

/// Puts `text` in the active tab's path as the Tasks list's search, in place of the current entry; a path
/// that already searches for it stays as it is.
pub(super) fn follow_search(navigation: &Navigation, text: &str) {
  let Route::Project(project, ProjectPage::Tasks(list)) = navigation.route_now() else {
    return;
  };
  if list.search().unwrap_or_default() != text {
    navigation.replace(Route::Project(project, ProjectPage::Tasks(list.with_search(text))));
  }
}
