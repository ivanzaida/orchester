//! The Modules list (no frame yet, ORC-48; laid out as the Tasks list's header over a Usage table): the title, what
//! modules are and *New module*, the editor while a module is made, and a row per module, oldest first, with its
//! dates, its state, its tasks and its progress as a meter and a share; a row opens the module's page.

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
use orchester_workspace::modules::data::{ModuleSummary, summaries};

use super::{Editing, editor, words};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ProjectPage, Route},
  screens::usage::table::{Cell, Density, Width, column, opening, row, table},
  theme::{ExtraSpacing, ExtraTypography, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    meter::{MeterTone, meter},
    state::{EmptyState, load_error, loading},
  },
};

/// The columns of a module's row besides its name.
const DATES_COLUMN: f32 = 180.0;
const STATE_COLUMN: f32 = 80.0;
const TASKS_COLUMN: f32 = 64.0;
const PROGRESS_COLUMN: f32 = 220.0;

#[derive(Clone, PartialEq)]
pub struct ListProps {
  pub project: ShownProject,
}

impl DevtoolsInspectable for ListProps {}

pub struct ModulesList {
  editing: Signal<Editing>,
}

impl Component for ModulesList {
  type Props = ListProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      editing: ctx.signal(Editing::Closed),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<ListProps>().project.clone();
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
      .child(self.header(ctx));
    let page = match self.editing.get() {
      Editing::New => page.child(ctx.mount_keyed::<editor::ModuleEditor>(
        "module-editor-new",
        editor::EditorProps {
          project: project.clone(),
          module: None,
          editing: self.editing.clone(),
        },
      )),
      Editing::Closed | Editing::Edit => page,
    };
    let content = match project.feed.tasks.get() {
      TasksState::Ready(tasks) => {
        let listed = summaries(&tasks.modules, &tasks.tasks);
        if listed.is_empty() {
          empty(ctx)
        } else {
          rows(ctx, &project, &listed)
        }
      }
      TasksState::Loading => loading("modules-loading", &ctx.t("modules.reading")).into(),
      TasksState::Failed(error) => {
        let feed = project.feed.clone();
        let body = message_text(ctx, &error);
        load_error(
          ctx,
          "modules-load-error",
          (&ctx.t("modules.readFailed"), &body),
          move || feed.saved(),
        )
      }
    };
    Column::new()
      .id("screen-modules")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(ScrollVertical::new(page.child(content)).id("modules-scroll").flex(1.0))
  }
}

impl ModulesList {
  fn header(&self, ctx: &Ctx) -> Row {
    let editing = self.editing.clone();
    let new = ActionButton::new("modules-new", ButtonKind::Primary, ctx.t("modules.newModule"))
      .icon(Icon::Plus)
      .on_click(move || editing.set(Editing::New));
    Row::new()
      .id("modules-header")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::End)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Snug.role())
          .child(
            Text::new(&ctx.t("modules.title"))
              .id("modules-title")
              .variant(ExtraTypography::Display.role()),
          )
          .child(
            Text::new(&ctx.t("modules.description"))
              .variant(TypographyStyle::Body)
              .color(PaletteColor::TextSecondary),
          ),
      )
      .child(new)
  }
}

fn empty(ctx: &Ctx) -> Element {
  EmptyState::new(
    "modules-empty",
    Icon::Target,
    ctx.t("modules.empty.title"),
    ctx.t("modules.empty.body"),
  )
  .into()
}

fn rows(ctx: &mut Ctx, project: &ShownProject, listed: &[ModuleSummary]) -> Element {
  let columns = [
    column(ctx.t("modules.columns.name"), Width::Flex(1.0), false),
    column(ctx.t("modules.columns.dates"), Width::Px(DATES_COLUMN), false),
    column(ctx.t("modules.columns.state"), Width::Px(STATE_COLUMN), false),
    column(ctx.t("modules.columns.tasks"), Width::Px(TASKS_COLUMN), true),
    column(ctx.t("modules.columns.progress"), Width::Px(PROGRESS_COLUMN), false),
  ];
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let rows = listed
    .iter()
    .enumerate()
    .map(|(index, summary)| {
      let details = summary.module.details();
      let cells = vec![
        Cell::Text(details.name().as_str().into()),
        Cell::Muted(words::dates(ctx, details)),
        Cell::Muted(words::state(ctx, details.state())),
        Cell::Text(summary.tasks.to_string().into()),
        Cell::Element(progress(ctx, &format!("modules-row-{index}-progress"), summary)),
      ];
      let open = navigation.to(Route::Project(id, ProjectPage::Module(summary.module.id())));
      opening(
        row(&format!("modules-row-{index}"), &columns, Density::List, cells)
          .describe("module", details.name().as_str()),
        open,
      )
    })
    .collect();
  table(ctx, "modules-table", &columns, Density::List, rows)
}

/// A module's progress: a meter of its done work over its scope, and the share done.
fn progress(ctx: &Ctx, id: &str, summary: &ModuleSummary) -> Element {
  let (done, scope) = (u64::from(summary.progress.done), u64::from(summary.progress.scope));
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .child(meter(&format!("{id}-meter"), (done, scope.max(1)), MeterTone::Plain)),
    )
    .child(
      Text::new(&words::percent(ctx, summary.progress).unwrap_or_else(|| ctx.t("modules.burndown.ahead")))
        .variant(TypographyStyle::Mono)
        .nowrap(),
    )
    .into()
}
