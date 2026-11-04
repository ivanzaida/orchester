//! Context › Code index (`Code index` `EORXu` in `03 · Context`; empty in `09` `mE5oU`): the workspace's setting with
//! its switch, a row per revision's index with its state and Rebuild or Retry, and the search tool with the ripgrep
//! install. The setting, the builds and the search tool are read through their queries: each shows its last read when
//! the tab opens, the builds and the search tool read again behind it.
mod list;
mod search;

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  components::{Column, Text},
  core::Signal,
  node::Element,
};
use orchester_core::revision::Revision;
use orchester_workspace::{
  code_index::{self, IndexList, RipgrepCheck},
  messages::Message,
  tasks::outside::Outside,
};

use super::{
  TabProps,
  kit::{EmptyRow, Group, SettingRow},
  work::{self, Change, Read, Save},
};
use crate::{
  queries::{self, Again, Kept, Session, project},
  services::Services,
  theme::ExtraTypography,
  ui::{
    field::field_error,
    icon::Icon,
    message::message_text,
    skeleton,
    toggle::{Toggle, ToggleKind},
  },
};

/// How long the bones of what the group has not read yet are (`Code index` in `L04L` `dFXLk`).
const INDEX_HINT_BONE: f32 = 150.0;
const REBUILD_BONE: f32 = 84.0;
const TOOL_HINT_BONE: f32 = 110.0;
const TOOL_BONE: f32 = 44.0;

#[derive(Clone, PartialEq)]
struct Listed(IndexList);
impl DevtoolsInspectable for Listed {}
#[derive(Clone, PartialEq)]
struct Checked(RipgrepCheck);
impl DevtoolsInspectable for Checked {}
/// *Rebuild* or *Retry* of the index of a revision, which the project's runner carries out.
type Rebuild = FutureAction<Revision, (), Message>;

pub(super) struct CodeIndex {
  save: Save,
  rebuild: Rebuild,
  /// What the page's last save read back, until the settings' query has it.
  kept: Kept<Read>,
  list_revision: Signal<u64>,
  check_revision: Signal<u64>,
  /// The builds' and the search tool's asks to read again, each of which invalidates its query.
  list_again: Again,
  check_again: Again,
  tick: Signal<u64>,
  watching: Arc<AtomicBool>,
  after_install: Arc<AtomicBool>,
}
impl Component for CodeIndex {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      rebuild: rebuild_action(ctx),
      kept: ctx.props::<TabProps>().kept.clone(),
      list_revision: ctx.signal(0),
      check_revision: ctx.signal(0),
      list_again: Again::live(),
      check_again: Again::live(),
      tick: ctx.signal(0),
      watching: Arc::new(AtomicBool::new(false)),
      after_install: Arc::new(AtomicBool::new(false)),
    };
    let save = tab.save.clone();
    let rebuild = tab.rebuild.clone();
    work::follow(ctx, &save, ctx.props::<TabProps>().status.clone(), tab.kept.clone());
    let revision = tab.list_revision.clone();
    ctx.watch(&rebuild.state(), move |_| revision.update(|r| *r += 1));
    tab.watch(ctx);
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = self.save.clone();
    let rebuild = self.rebuild.clone();
    self.tick.get();
    let loaded = work::load(ctx, props.project.open.repo_root.clone());
    let list = self.list(ctx, &props);
    let check = self.check(ctx, &props);
    let install = code_index::install_state();
    self.watching.store(
      list.data.as_ref().is_some_and(|list| list.0.building())
        || rebuild.state().get().is_pending()
        || matches!(install, Some(code_index::Install::Running { .. })),
      Ordering::Relaxed,
    );
    let Some(read) = loaded.shown(&self.kept, save.state().get().is_pending()) else {
      return work::pending(ctx, &loaded, bones);
    };
    let on = read.0.code_index.is_on();
    let mut group =
      Group::new("settings-index-section", ctx.t("project_settings.indexHeading")).row(setting(ctx, &props, on, &save));
    if on {
      if let Some(total) = list::total(ctx, &list) {
        group = group.end(
          Text::new(&total)
            .id("settings-index-total")
            .variant(ExtraTypography::Note.role()),
        );
      }
      group = group.rows(list::rows(ctx, &list, &rebuild)).rows(search::rows(
        ctx,
        &check,
        code_index::install_state().as_ref(),
        (&self.check_revision, &self.tick, &self.after_install),
      ));
    } else {
      group = group.row(EmptyRow::new(
        "settings-index-off",
        Icon::CircleOff,
        ctx.t("project_settings.indexOffTitle"),
        ctx.t("project_settings.indexOffNote"),
      ));
    }
    let errors = [
      work::error(ctx, &save),
      loaded
        .state
        .error
        .map(|e| field_error("settings-index-load-error", &message_text(ctx, &e)).into()),
      rebuild
        .state()
        .get()
        .error
        .map(|e| field_error("settings-index-rebuild-error", &message_text(ctx, &e)).into()),
    ];
    for error in errors.into_iter().flatten() {
      group = group.after(error);
    }
    group.into()
  }
}
impl CodeIndex {
  fn watch(&self, ctx: &mut Ctx) {
    let (watching, tick, list, check) = (
      self.watching.clone(),
      self.tick.clone(),
      self.list_revision.clone(),
      self.check_revision.clone(),
    );
    let was_running = AtomicBool::new(false);
    let after_install = self.after_install.clone();
    ctx
      .create_interval(Duration::from_secs(1), move || {
        let running = matches!(code_index::install_state(), Some(code_index::Install::Running { .. }));
        let finished = was_running.swap(running, Ordering::Relaxed);
        if !running && (after_install.swap(false, Ordering::Relaxed) || finished) {
          check.update(|r| *r += 1);
          tick.update(|r| *r += 1);
        }
        if watching.load(Ordering::Relaxed) {
          tick.update(|r| *r += 1);
          list.update(|r| *r += 1);
        }
      })
      .start();
  }
  /// The index's builds through their query, its last read at once and read again behind it when the tab opens; read
  /// again at the tab's poll, a rebuild, the project's screens' saves, a change of the setting and the runs' state,
  /// and, while a run runs and no build is followed, each of the runs' changes (a transcript line may be the runner
  /// starting one). The project's loader reads them again on the runs' state too (`queries::changes::RUNS`). Each read
  /// the tab asks for is logged on the window's thread, as it asks.
  fn list(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Listed, Message> {
    let feed = &props.project.feed;
    let asked = queries::asked(&(
      self.list_revision.get(),
      feed.run_changes(feed.running.get() && !self.watching.load(Ordering::Relaxed)),
      feed.revision.get(),
      feed.outside.get().of(Outside::CodeIndex),
    ));
    let session = Session::of(ctx);
    let query = ctx.query(project::index_builds(session, props.project.open.repo_root.clone()));
    if self.list_again.follow(asked, &query) {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "code index");
    }
    let FutureState { status, data, error } = queries::state(&query);
    FutureState {
      status,
      data: data.map(Listed),
      error,
    }
  }
  /// Whether ripgrep is there, through its query; read again after an install and at *Check again*.
  fn check(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Checked, Message> {
    let session = Session::of(ctx);
    let query = ctx.query(project::ripgrep(session, props.project.open.repo_root.clone()));
    self.check_again.follow(self.check_revision.get(), &query);
    let FutureState { status, data, error } = queries::state(&query);
    FutureState {
      status,
      data: data.map(Checked),
      error,
    }
  }
}

/// The group while the setting is read (`L04L` `dFXLk`), the tab's third: *Use code index* with its switch's bone, the
/// index's line, state and Rebuild as bones, the search tool's line and name as bones.
fn bones(ctx: &mut Ctx) -> Column {
  let setting = SettingRow::new("settings-index-use-reading", ctx.t("project_settings.useIndex"))
    .info(ctx.t("project_settings.indexInfo"))
    .hint(ctx.t("project_settings.indexOnHint"))
    .control(skeleton::switch())
    .render(ctx);
  let index = SettingRow::new("settings-index-list-reading", ctx.t("project_settings.indexRow"))
    .hint_reading(INDEX_HINT_BONE)
    .control(skeleton::button(REBUILD_BONE))
    .render(ctx);
  let tool = SettingRow::new("settings-ripgrep-reading", ctx.t("project_settings.searchTool"))
    .info(ctx.t("project_settings.ripgrepDetail"))
    .hint_reading(TOOL_HINT_BONE)
    .control(skeleton::line(ctx, ExtraTypography::Note.role(), TOOL_BONE))
    .render(ctx);
  skeleton::lower_group(
    1,
    Group::new("settings-index-section-reading", ctx.t("project_settings.indexHeading"))
      .row(setting)
      .row(index)
      .row(tool),
  )
}

/// *Use code index* (`Use code index` `LEaze`): its switch, saved at once.
fn setting(ctx: &mut Ctx, props: &TabProps, on: bool, save: &Save) -> Element {
  let start = work::start(save, props.project.open.repo_root.clone(), Change::Index(!on));
  let toggle = Toggle::new(
    "settings-code-index",
    ToggleKind::Switch,
    on,
    ctx.t(if on {
      "project_settings.indexOn"
    } else {
      "project_settings.indexOff"
    }),
  )
  .hide_label();
  let toggle = if save.state().get().is_pending() {
    toggle
  } else {
    toggle.on_change(move |_| start())
  };
  SettingRow::new("settings-index-use", ctx.t("project_settings.useIndex"))
    .info(ctx.t("project_settings.indexInfo"))
    .hint(ctx.t(if on {
      "project_settings.indexOnHint"
    } else {
      "project_settings.indexDirect"
    }))
    .control(toggle)
    .render(ctx)
}
/// *Rebuild* and *Retry*, which the project's runner carries out.
fn rebuild_action(ctx: &mut Ctx) -> Rebuild {
  let (runners, open) = (Services::of(ctx).runners, ctx.props::<TabProps>().project.open.clone());
  ctx.future_action(move |revision: Revision| {
    let (runners, open) = (runners.clone(), open.clone());
    async move { code_index::rebuild(&runners, &open, revision).await }
  })
}
