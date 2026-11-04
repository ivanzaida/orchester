//! A team module's page (`01e · Module · member` `FuHYN`, `06d · Module · team data unavailable` `B7Mbe`, ORC-717):
//! the module, its tasks (the task list read with its module filter) and the person's standing, read from the server
//! through the session's lurq queries (`crate::team::pages`). The header has no New task in module and no *More
//! actions*: changing a team module is an Admin's and comes with the team's actions. The Summary is the local one's
//! layout with, for a member, one calm line naming who changes the module over it (`Read-only line` `HBmrp`), and the
//! properties as text ([`super::team_properties`]). While the team's data cannot be read, the status line says since
//! when (`Unavailable` `losIR`), the outage card stands in the body (`crate::team::outage`) and, 24 under it, a line
//! names the module's tasks still running on this computer (`s97Htc`), from what was last read of its tasks and this
//! computer's runs; no module data is shown as current.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ModuleId, ProjectId},
  run::TaskPhase,
};
use orchester_workspace::tasks::list::Latest;

use super::{
  header,
  page::{ModuleScreen, back_only, body_padding, missing, padded, tabs},
  summary::{burndown_line, description},
  tasks, team_properties, words,
};
use crate::{
  project::{ShownProject, TasksState},
  routes::ModuleTab,
  screens::settings::orchester::moment,
  team::{
    TeamFailure, lock, outage,
    pages::{TeamModule, team_module},
    queries::{TaskListKey, TeamTasks, team_tasks},
    view::{self, Standing},
  },
  theme::{ExtraSpacing, HubText, IconSize, TaskSpacing},
  ui::icon::{Icon, icon},
};

/// What the page shows of a team module.
enum Team {
  Reading,
  Missing,
  /// Why the module cannot be read, with what was read of it before, which only names it.
  Failed(TeamFailure, Option<Arc<TeamModule>>, Option<Arc<TeamTasks>>),
  Ready(Arc<TeamModule>, Arc<TeamTasks>, Standing),
}

impl ModuleScreen {
  /// The page of the team module `module` of `project` on `tab`.
  pub(super) fn team(&self, ctx: &mut Ctx, project: &ShownProject, module: ModuleId, tab: ModuleTab) -> Column {
    view::provide_client(ctx);
    let place = view::place(ctx, project);
    let id = project.open.project.id();
    let (top, body) = match read(ctx, project, module) {
      Team::Reading => super::reading::page(ctx, (id, module), (tab, false)),
      Team::Missing => (back_only(ctx, id), padded(missing(ctx))),
      Team::Failed(failure, known, tasks) => {
        let name = known
          .as_ref()
          .map(|known| known.module.details().name().as_str().to_owned());
        let top = unavailable_header(ctx, (id, module, name.as_deref()), &failure, tab);
        let body = unavailable_body(ctx, (project, &place), (&failure, name.as_deref()), tasks.as_deref());
        (top, body)
      }
      Team::Ready(shown, tasks, standing) => {
        let top = ready_header(ctx, (id, &shown), tab);
        let body = match tab {
          ModuleTab::Summary => self.team_summary(ctx, id, (&shown, &tasks), &standing),
          ModuleTab::Burndown => padded(
            Text::new(&ctx.t("modules.team.page.noBurndown"))
              .id("module-team-no-burndown")
              .variant(TypographyStyle::Description)
              .into(),
          ),
        };
        (top, body)
      }
    };
    Column::new()
      .id("screen-modules")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(top)
      .child(body)
  }

  /// The Summary: the lock line for a member, the description, the tasks and the burndown line, the properties beside.
  fn team_summary(
    &self,
    ctx: &mut Ctx,
    project: ProjectId,
    (shown, tasks): (&TeamModule, &TeamTasks),
    standing: &Standing,
  ) -> Element {
    let lock = standing.member().then(|| {
      let words = lock::lock_words(
        ctx,
        standing.admins.as_deref(),
        ("modules.team.page.readOnly", "modules.team.page.readOnlyUnnamed"),
      );
      lock::locked("module-team-read-only", &words)
    });
    let burndown = shown
      .forecast
      .as_ref()
      .map(|forecast| burndown_line(ctx, (project, shown.module.id()), (shown.progress, forecast)));
    let column = Column::new()
      .id("module-summary-column")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(TaskSpacing::Sections.role())
      .with_children(lock)
      .child(description(ctx, &shown.module))
      .child(tasks::section(ctx, project, &tasks.tasks, &self.toggled))
      .with_children(burndown);
    let body = Row::new()
      .id("module-summary-body")
      .width(Dimension::Pct(100.0))
      .padding(body_padding())
      .spacing(TaskSpacing::Columns.role())
      .align_items(Alignment::Start)
      .child(column)
      .child(team_properties::column(ctx, shown));
    Column::new()
      .id("module-summary")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(ScrollVertical::new(body).id("module-scroll").flex(1.0))
      .into()
  }
}

/// The module, its tasks and the person's standing as the server reads them.
fn read(ctx: &mut Ctx, project: &ShownProject, module: ModuleId) -> Team {
  let source = match view::source(ctx, project) {
    Ok(Some(source)) => source,
    Ok(None) => return Team::Reading,
    Err(failure) => return Team::Failed(failure, None, None),
  };
  let standing = view::standing(ctx, &source);
  let read = ctx.query(team_module(source.clone(), module));
  let key = TaskListKey {
    modules: vec![Some(module)],
    ..TaskListKey::default()
  };
  let listed = ctx.query(team_tasks(source, key));
  let found = read.data();
  let known = found.as_deref().and_then(|found| found.clone()).map(Arc::new);
  let failure = [read.error(), listed.error()]
    .into_iter()
    .flatten()
    .next()
    .map(|failure| (*failure).clone())
    .or_else(|| standing.as_ref().err().cloned());
  if let Some(failure) = failure {
    return Team::Failed(failure, known, listed.data());
  }
  match (found, listed.data(), standing) {
    (Some(found), ..) if found.is_none() => Team::Missing,
    (Some(_), Some(tasks), Ok(Some(standing))) => {
      known.map_or(Team::Missing, |shown| Team::Ready(shown, tasks, standing))
    }
    _ => Team::Reading,
  }
}

/// `← Modules / name`, the name, its state and facts, and the tabs.
fn ready_header(ctx: &mut Ctx, (project, shown): (ProjectId, &TeamModule), tab: ModuleTab) -> Element {
  let details = shown.module.details();
  let mut status = vec![header::state_chip(ctx, "module-state", details.state())];
  if let Some(forecast) = &shown.forecast {
    let facts = Text::new(&words::facts(ctx, details, shown.progress, forecast))
      .id("module-facts")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap();
    status.push(facts.into());
  }
  header::header(header::Header {
    id: "module-header",
    back: Some(header::back_row(ctx, project, details.name().as_str())),
    title: header::title(ctx, "module-title", details.name().as_str()),
    actions: Vec::new(),
    status: header::status_line("module-status-row", status),
    tabs: tabs(ctx, (project, shown.module.id()), tab),
  })
}

/// The header while the team's data cannot be read: the module's name as last read, and since when the server has not
/// answered in place of its state and facts.
fn unavailable_header(
  ctx: &mut Ctx,
  (project, module, name): (ProjectId, ModuleId, Option<&str>),
  failure: &TeamFailure,
  tab: ModuleTab,
) -> Element {
  let Some(name) = name else {
    return back_only(ctx, project);
  };
  let words = match failure {
    TeamFailure::Unavailable { since } => {
      ctx.t_args("modules.team.page.unavailableSince", [("time", moment(ctx, *since))])
    }
    TeamFailure::SignedOut | TeamFailure::NotMember | TeamFailure::Refused => ctx.t("tasks.team.unavailable.title"),
  };
  let status = Row::new()
    .id("module-team-unavailable-since")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CloudOff, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  header::header(header::Header {
    id: "module-header",
    back: Some(header::back_row(ctx, project, name)),
    title: header::title(ctx, "module-title", name),
    actions: Vec::new(),
    status: header::status_line("module-status-row", vec![status.into()]),
    tabs: tabs(ctx, (project, module), tab),
  })
}

/// The outage card and, 24 under it, the module's tasks still running on this computer.
fn unavailable_body(
  ctx: &mut Ctx,
  (project, place): (&ShownProject, &outage::Place),
  (failure, name): (&TeamFailure, Option<&str>),
  tasks: Option<&TeamTasks>,
) -> Element {
  let title = match name {
    Some(name) => ctx.t_args("modules.team.page.unavailableTitle", [("module", name)]),
    None => ctx.t("modules.team.page.unavailableTitleUnnamed"),
  };
  let unread = outage::Unread {
    id: "module-team-unavailable",
    title,
    unavailable: "modules.team.page.unavailableBody",
  };
  let card = outage::card(ctx, &unread, (failure, place, project.open.project.name().as_str()));
  let running = running_line(ctx, project, tasks);
  ScrollVertical::new(
    Column::new()
      .id("module-team-outage")
      .width(Dimension::Pct(100.0))
      .padding(body_padding())
      .spacing(SpacingSize::Xl)
      .child(card)
      .with_children(running),
  )
  .id("module-scroll")
  .flex(1.0)
  .into()
}

/// *KON-12, one of its tasks, is still running on this computer; open it from Running in the sidebar.*; `None` when
/// none of the module's tasks as last read runs here.
fn running_line(ctx: &Ctx, project: &ShownProject, tasks: Option<&TeamTasks>) -> Option<Element> {
  let (TasksState::Ready(local), Some(tasks)) = (project.feed.tasks.get(), tasks) else {
    return None;
  };
  let keys: Vec<String> = tasks
    .tasks
    .iter()
    .filter(|task| {
      matches!(local.phases.get(&task.id()), Some(Latest::Run(latest))
        if matches!(latest.phase, TaskPhase::Running | TaskPhase::Waiting))
    })
    .map(|task| task.key().to_string())
    .collect();
  let words = match keys.as_slice() {
    [] => return None,
    [one] => ctx.t_args("modules.team.page.runningOne", [("task", one.as_str())]),
    more => ctx.t_args(
      "modules.team.page.runningOther",
      [("tasks", more.join(&ctx.t("tasks.team.separator")))],
    ),
  };
  Some(
    Row::new()
      .id("module-team-running")
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(ExtraSpacing::Tight.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&words)
          .id("module-team-running-text")
          .variant(HubText::TeamLine.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into(),
  )
}
