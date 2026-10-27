//! A module's page (`02 · Module · Summary` `KpLYv`, `03 · Module · Burndown` `YyKzx`), in the task page's frame: the
//! Modules header ([`super::header`]) with `← Modules / name`, the name with *New task in module* and *More actions*
//! ([`super::more`]), the status line (the state chip, then its dates, its progress and its forecast in 12
//! `text_muted`) and the tabs Summary and Burndown, each a place of its own (`/modules/<id>`,
//! `/modules/<id>/burndown`); under them the tab: the Summary ([`super::summary`]) or the burndown card
//! ([`super::burndown`]). The page is read off the UI thread, again whenever the project's tasks are read again; until
//! it is, and when it cannot be or the module no longer exists, the way back stays over what the body says.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ModuleId, ProjectId},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  modules::data::ModulePage,
};

use super::{burndown, header, more, save, summary, words};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ModuleListTab, ModuleTab, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize, TaskSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    state::{EmptyState, load_error, loading},
    tabs::{PageTab, page_tabs},
  },
};

#[derive(Clone, PartialEq)]
pub struct PageProps {
  pub project: ShownProject,
  pub module: ModuleId,
  pub tab: ModuleTab,
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
  /// The burndown reads as a table; kept as the tabs switch.
  table: Signal<bool>,
  retry: Signal<u64>,
  menu: more::MenuPlace,
  /// *Close module* and *Reopen module*.
  save: save::SaveAction,
}

impl Component for ModuleScreen {
  type Props = PageProps;

  fn create(ctx: &mut Ctx) -> Self {
    let save = save::action(ctx);
    let feed = ctx.props::<PageProps>().project.feed.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        feed.saved();
      }
    });
    Self {
      table: ctx.signal(false),
      retry: ctx.signal(0),
      menu: more::MenuPlace::new(ctx),
      save,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let PageProps { project, module, tab } = ctx.props::<PageProps>().clone();
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
    let id = project.open.project.id();
    // A failed read keeps the page read before it: it is not shown (nor edited) as if it were current.
    let failed = read.error.as_ref().filter(|_| read.is_rejected());
    let (top, body) = match (failed, &read.data) {
      (Some(ReadFailed(message)), _) => {
        let retry = self.retry.clone();
        let words = message_text(ctx, message);
        let failed = load_error(
          ctx,
          "module-load-error",
          (&ctx.t("modules.readFailed"), &words),
          move || {
            retry.update(|count| *count += 1);
          },
        );
        (back_only(ctx, id), padded(failed))
      }
      (None, Some(Read(Some(page)))) => (
        self.header(ctx, (&project, page), tab),
        self.body(ctx, &project, page, tab),
      ),
      (None, Some(Read(None))) => (back_only(ctx, id), padded(missing(ctx))),
      (None, None) => (
        back_only(ctx, id),
        padded(loading("module-loading", &ctx.t("modules.page.reading")).into()),
      ),
    };
    Column::new()
      .id("screen-modules")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(top)
      .child(body)
  }
}

impl ModuleScreen {
  fn header(&self, ctx: &mut Ctx, (project, page): (&ShownProject, &Arc<ModulePage>), tab: ModuleTab) -> Element {
    let id = project.open.project.id();
    let details = page.module.details();
    let navigation = Navigation::of(ctx);
    let new_task = ActionButton::new("module-new-task", ButtonKind::Primary, ctx.t("modules.page.newTask"))
      .icon(Icon::Plus)
      .opens(navigation.to(Route::Project(id, ProjectPage::NewTaskInModule(page.module.id()))));
    let actions = vec![
      new_task.into(),
      more::more(ctx, (project, page), &self.menu, &self.save),
    ];
    let chip = header::state_chip(ctx, "module-state", details.state());
    let facts = Text::new(&words::facts(ctx, details, page.progress, &page.forecast))
      .id("module-facts")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap();
    let mut status = vec![chip, facts.into()];
    status.extend(self.refused(ctx));
    header::header(header::Header {
      id: "module-header",
      back: Some(header::back_row(ctx, id, details.name().as_str())),
      title: header::title(ctx, "module-title", details.name().as_str()),
      actions,
      status: header::status_line("module-status-row", status),
      tabs: tabs(ctx, (id, page.module.id()), tab),
    })
  }

  /// Why *Close module* or *Reopen module* was refused, at the status line's end.
  fn refused(&self, ctx: &Ctx) -> Option<Element> {
    let state = self.save.state().get();
    let save::Refused(message) = state.error.as_ref().filter(|_| state.is_rejected())?;
    Some(field_error("module-state-error", &message_text(ctx, message)).into())
  }

  fn body(&self, ctx: &mut Ctx, project: &ShownProject, page: &Arc<ModulePage>, tab: ModuleTab) -> Element {
    match tab {
      ModuleTab::Summary => ctx.mount_keyed::<summary::ModuleSummary>(
        "module-summary",
        summary::SummaryProps {
          project: project.clone(),
          page: page.clone(),
        },
      ),
      ModuleTab::Burndown => {
        let card = burndown::card(ctx, page, &self.table);
        ScrollVertical::new(
          Column::new()
            .id("module-burndown-body")
            .width(Dimension::Pct(100.0))
            .padding(body_padding())
            .child(card),
        )
        .id("module-scroll")
        .flex(1.0)
        .into()
      }
    }
  }
}

/// Summary and Burndown.
fn tabs(ctx: &mut Ctx, (project, module): (ProjectId, ModuleId), chosen: ModuleTab) -> Element {
  let navigation = Navigation::of(ctx);
  let tabs: Vec<PageTab> = ModuleTab::ALL
    .into_iter()
    .map(|tab| {
      let label = ctx.t(match tab {
        ModuleTab::Summary => "modules.page.tabs.summary",
        ModuleTab::Burndown => "modules.page.tabs.burndown",
      });
      let open = navigation.to(Route::Project(project, ProjectPage::Module(module, tab)));
      PageTab::to(format!("module-tab-{}", tab.name()), label, tab == chosen, open)
    })
    .collect();
  page_tabs(ctx, "module-tabs", tabs).into()
}

/// The page's body under its header: 24 over it, 28 at its sides and under it.
pub fn body_padding() -> Padding {
  Padding::new()
    .top(SpacingSize::Xl)
    .left(TaskSpacing::Side.role())
    .right(TaskSpacing::Side.role())
    .bottom(TaskSpacing::Foot.role())
}

fn padded(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(body_padding())
    .child(content)
    .into()
}

/// `← Modules` alone, over a page that is read, failed or gone (`Crumb` `XeoEE`).
fn back_only(ctx: &mut Ctx, project: ProjectId) -> Element {
  let navigation = Navigation::of(ctx);
  let list = Route::Project(project, ProjectPage::Modules(ModuleListTab::default()));
  let words = ctx.t("modules.page.back");
  let back = opening(navigation.to(list))
    .id("module-back")
    .describe("a11y_name", words.clone())
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  Row::new()
    .id("module-back-row")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(TaskSpacing::Header.role())
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role()),
    )
    .child(back)
    .into()
}

fn missing(ctx: &Ctx) -> Element {
  EmptyState::new(
    "module-not-found",
    Icon::SearchX,
    ctx.t("modules.page.notFound"),
    ctx.t("modules.page.notFoundBody"),
  )
  .unframed()
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
