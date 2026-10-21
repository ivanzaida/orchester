//! A module's page (no frame yet, ORC-48; laid out as a Usage card under a Tasks-style header): back to Modules,
//! the module's name, state, dates and description with *Edit*, its progress, the burndown as a chart or a table
//! ([`super::burndown`]), and its tasks, each opening its page. The page is read off the UI thread, again whenever
//! the project's tasks are read again.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ModuleId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::{self, Message},
  modules::data::ModulePage,
};

use super::{Editing, burndown, editor, words};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ProjectPage, Route, TaskView},
  screens::usage::table::{Cell, Density, Width, column, opening, row, table},
  theme::{ExtraSpacing, ExtraTypography, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    state::{EmptyState, load_error, loading},
    status::workflow_chip,
  },
};

/// The key and status columns of the module's tasks.
const KEY_COLUMN: f32 = 96.0;
const STATUS_COLUMN: f32 = 140.0;

#[derive(Clone, PartialEq)]
pub struct PageProps {
  pub project: ShownProject,
  pub module: ModuleId,
}

impl DevtoolsInspectable for PageProps {}

/// The page as read.
#[derive(Clone)]
pub struct Read(pub Option<Arc<ModulePage>>);

impl PartialEq for Read {
  fn eq(&self, other: &Self) -> bool {
    match (&self.0, &other.0) {
      (Some(left), Some(right)) => Arc::ptr_eq(left, right),
      (None, None) => true,
      _ => false,
    }
  }
}

impl DevtoolsInspectable for Read {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFailed(pub Message);

impl DevtoolsInspectable for ReadFailed {}

pub struct ModuleScreen {
  editing: Signal<Editing>,
  /// The burndown reads as a table.
  table: Signal<bool>,
  retry: Signal<u64>,
}

impl Component for ModuleScreen {
  type Props = PageProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      editing: ctx.signal(Editing::Closed),
      table: ctx.signal(false),
      retry: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let PageProps { project, module } = ctx.props::<PageProps>().clone();
    // Read again whenever the project's tasks are read again, which every save and run change does.
    let revision = match project.feed.tasks.get() {
      TasksState::Ready(tasks) => Arc::as_ptr(&tasks.tasks) as usize,
      TasksState::Loading | TasksState::Failed(_) => 0,
    };
    let read = read(
      ctx,
      project.open.repo_root.clone(),
      module,
      (revision, self.retry.get()),
    );
    // A failed read keeps the page read before it: it is not shown (nor edited) as if it were current.
    let failed = read.error.as_ref().filter(|_| read.is_rejected());
    let content = match (failed, &read.data) {
      (Some(ReadFailed(message)), _) => {
        let retry = self.retry.clone();
        let body = message_text(ctx, message);
        vec![load_error(
          ctx,
          "module-load-error",
          (&ctx.t("modules.readFailed"), &body),
          move || {
            retry.update(|count| *count += 1);
          },
        )]
      }
      (None, Some(Read(Some(page)))) => self.page(ctx, &project, page),
      (None, Some(Read(None))) => vec![missing(ctx)],
      (None, None) => vec![loading("module-loading", &ctx.t("modules.page.reading")).into()],
    };
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Roomy.role())
          .bottom(SpacingSize::Section)
          .left(TasksSpacing::Page.role())
          .right(TasksSpacing::Page.role()),
      )
      .spacing(SpacingSize::Lg)
      .child(back(ctx, project.open.project.id()))
      .with_children(content);
    Column::new()
      .id("screen-modules")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(ScrollVertical::new(page).id("module-scroll").flex(1.0))
  }
}

impl ModuleScreen {
  fn page(&self, ctx: &mut Ctx, project: &ShownProject, page: &Arc<ModulePage>) -> Vec<Element> {
    let mut parts = vec![self.header(ctx, page)];
    if self.editing.get() == Editing::Edit {
      parts.push(ctx.mount_keyed::<editor::ModuleEditor>(
        &format!("module-editor-{}", page.module.id()),
        editor::EditorProps {
          project: project.clone(),
          module: Some(Arc::new(page.module.clone())),
          editing: self.editing.clone(),
        },
      ));
    }
    parts.push(burndown::card(ctx, page, &self.table));
    parts.push(tasks(ctx, project, page));
    parts
  }

  fn header(&self, ctx: &Ctx, page: &ModulePage) -> Element {
    let details = page.module.details();
    let editing = self.editing.clone();
    let edit = ActionButton::new("module-edit", ButtonKind::Secondary, ctx.t("modules.page.edit"))
      .icon(Icon::Pencil)
      .on_click(move || editing.set(Editing::Edit));
    let facts = [
      words::state(ctx, details.state()),
      words::dates(ctx, details),
      words::progress(ctx, page.progress),
    ]
    .join(" · ");
    let description = (!details.description().is_empty()).then(|| {
      Text::new(details.description())
        .id("module-description-text")
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary)
    });
    Row::new()
      .id("module-header")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::End)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Snug.role())
          .child(
            Text::new(details.name().as_str())
              .id("module-title")
              .variant(ExtraTypography::Display.role()),
          )
          .child(
            Text::new(&facts)
              .id("module-facts")
              .variant(TypographyStyle::Description),
          )
          .with_children(description),
      )
      .child(edit)
      .into()
  }
}

/// Back to the Modules list.
fn back(ctx: &mut Ctx, project: orchester_core::id::ProjectId) -> Element {
  let navigation = Navigation::of(ctx);
  ActionButton::new("module-back", ButtonKind::Ghost, ctx.t("modules.page.back"))
    .icon(Icon::ArrowLeft)
    .compact()
    .opens(navigation.to(Route::Project(project, ProjectPage::Modules)))
    .into()
}

fn missing(ctx: &Ctx) -> Element {
  EmptyState::new(
    "module-not-found",
    Icon::SearchX,
    ctx.t("modules.page.notFound"),
    ctx.t("modules.page.notFoundBody"),
  )
  .into()
}

/// The module's tasks: key, title and workflow status, each opening its page.
fn tasks(ctx: &mut Ctx, project: &ShownProject, page: &ModulePage) -> Element {
  let heading = Text::new(&ctx.t("modules.page.tasks"))
    .id("module-tasks-title")
    .variant(TypographyStyle::Title);
  let body = if page.tasks.is_empty() {
    Text::new(&ctx.t("modules.page.noTasks"))
      .id("module-no-tasks")
      .variant(TypographyStyle::Description)
      .into()
  } else {
    let columns = [
      column(ctx.t("modules.columns.key"), Width::Px(KEY_COLUMN), false),
      column(ctx.t("modules.columns.task"), Width::Flex(1.0), false),
      column(ctx.t("modules.columns.status"), Width::Px(STATUS_COLUMN), false),
    ];
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let rows = page
      .tasks
      .iter()
      .enumerate()
      .map(|(index, task)| {
        let cells = vec![
          Cell::Muted(task.key().to_string().into()),
          Cell::Text(task.details().title.as_str().into()),
          Cell::Element(
            workflow_chip(
              ctx,
              &format!("module-task-{index}-status"),
              task.organization().status(),
            )
            .into(),
          ),
        ];
        let open = navigation.to(Route::Project(
          id,
          ProjectPage::Task(task.key().clone(), TaskView::default()),
        ));
        opening(
          row(&format!("module-task-{index}"), &columns, Density::List, cells),
          open,
        )
      })
      .collect();
    table(ctx, "module-tasks", &columns, Density::List, rows)
  };
  Column::new()
    .id("module-tasks-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(heading)
    .child(body)
    .into()
}

/// Module `module` of the project at `root`, read off the UI thread again whenever `deps` change.
fn read<D>(ctx: &mut Ctx, root: PathBuf, module: ModuleId, deps: D) -> lurq::app::ctx::FutureState<Read, ReadFailed>
where
  D: Clone + PartialEq + Send + Sync + 'static,
{
  ctx
    .future((root, module, deps), |(root, module, _)| async move {
      background::run(move || {
        let db = orchester_store::ProjectDb::open(&root)?;
        ModulePage::read(&db, module, Timestamp::now())
      })
      .await
      .map_err(ReadFailed)?
      .map(|page| Read(page.map(Arc::new)))
      .map_err(|error| {
        tracing::error!(%error, "cannot read the module");
        ReadFailed(messages::store_error(&error))
      })
    })
    .state()
    .get()
}
