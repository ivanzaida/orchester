//! The tasks kept *Not shared* when the project joined (`A7PKfL`, `T1ahf8`): out of the shared list, behind the *Not
//! shared · 2* filter at the start of the applied filters' line, which shows only while there are any. With it on, the
//! list holds them alone, read-only, under a notice that says why they stayed; its views count them and every filter
//! applies to them. A search that finds only a *Not shared* task says so and offers *Show Not shared tasks*, the search
//! kept.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, task::Task};
use orchester_workspace::tasks::{
  browse::{self, Query, TaskSort},
  data::ProjectTasks,
  list::TaskTab,
};

use super::shared::Shared;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList},
  screens::settings::orchester::day_of,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    filter::FilterToggle,
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// The *Not shared* filter (`Not shared · 2` in `Xr4Wb`); `None` without such tasks.
pub(super) fn filter(ctx: &mut Ctx, (project, list): (ProjectId, &TaskList), count: usize) -> Option<Element> {
  if count == 0 {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let (on, shown) = (list.not_shared(), list.clone());
  let toggle = FilterToggle::new(
    "tasks-not-shared",
    ctx.t_args("tasks.notShared.filter", [("count", count.to_string())]),
    on,
    move || {
      let list = shown.clone().with_not_shared(!on);
      navigation.replace(Route::Project(project, ProjectPage::Tasks(list)));
    },
  );
  Some(toggle.into())
}

/// *2 tasks stayed on this computer when Kontur joined KTR* over the *Not shared* rows (`Notice` in `A7PKfL`).
pub(super) fn notice(ctx: &Ctx, (kept, name): (&[Task], &str), shared: Option<&Shared>) -> Element {
  let keys: Vec<String> = kept.iter().map(|task| task.key().to_string()).collect();
  let (key, server, day) = shared.map_or_else(
    || (String::new(), String::new(), String::new()),
    |shared| {
      let link = &shared.sharing.link;
      let server = shared
        .sharing
        .server
        .as_ref()
        .map(|(name, _)| name.clone())
        .unwrap_or_default();
      (link.key.as_str().to_owned(), server, day_of(ctx, link.joined_at))
    },
  );
  let title = if kept.len() == 1 {
    ctx.t_args("tasks.notShared.title.one", [("project", name), ("key", key.as_str())])
  } else {
    ctx.t_args(
      "tasks.notShared.title.other",
      [
        ("count", kept.len().to_string().as_str()),
        ("project", name),
        ("key", key.as_str()),
      ],
    )
  };
  let listed = keys.join(&ctx.t("tasks.notShared.and"));
  Notice::new("tasks-not-shared-notice", NoticeTone::Info, title)
    .icon(Icon::Archive)
    .body(ctx.t_args(
      "tasks.notShared.body",
      [("keys", listed), ("day", day), ("server", server), ("key", key)],
    ))
    .into()
}

/// The *Not shared* tasks a search with the filter off finds, which the shared rows never mix with.
pub(super) fn matching<'a>(tasks: &'a ProjectTasks, search: &str, separator: &str) -> Vec<&'a Task> {
  if search.trim().is_empty() || tasks.not_shared.is_empty() {
    return Vec::new();
  }
  let organization = orchester_core::task::organization::TaskFilter::default();
  let query = Query {
    text: search,
    pipeline: browse::PipelineFilter::default(),
    organization: &organization,
    separator,
  };
  let rows = browse::rows(&tasks.not_shared_view(), TaskTab::All, query, TaskSort::Manual);
  tasks
    .not_shared
    .iter()
    .filter(|task| rows.iter().any(|row| row.id == task.id()))
    .collect()
}

/// *No shared task has KON-15*: the search finds only a *Not shared* task; *Show Not shared tasks* turns the filter
/// on with the search kept (`Search finds only a Not shared task` in `T1ahf8`).
pub(super) fn only_not_shared(ctx: &mut Ctx, (project, list): (ProjectId, &TaskList), found: &[&Task]) -> Element {
  let search = list.search().unwrap_or_default();
  let detail: Arc<str> = match found {
    [one] => ctx.t_args(
      "tasks.notShared.foundOne",
      [
        ("key", one.key().to_string()),
        ("title", one.details().title.as_str().to_owned()),
      ],
    ),
    many => ctx.t_args("tasks.notShared.foundMany", [("count", many.len().to_string())]),
  };
  let navigation = Navigation::of(ctx);
  let shown = list.clone().with_not_shared(true);
  let show = ActionButton::new(
    "tasks-show-not-shared",
    ButtonKind::Ghost,
    ctx.t("tasks.notShared.show"),
  )
  .on_click(move || navigation.replace(Route::Project(project, ProjectPage::Tasks(shown.clone()))));
  Column::new()
    .id("tasks-only-not-shared")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Loading.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args("tasks.notShared.noShared", [("search", search)]))
        .id("tasks-only-not-shared-title")
        .variant(ExtraTypography::Strong.role()),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(
          Text::new(&detail)
            .id("tasks-only-not-shared-detail")
            .variant(TypographyStyle::Description),
        )
        .child(show),
    )
    .into()
}
