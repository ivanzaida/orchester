//! Context › Code index (`Code index` `EORXu` in `03 · Context`; empty in `09` `mE5oU`): the workspace's setting with
//! its switch, a row per revision's index with its state and Rebuild or Retry, and the search tool with the ripgrep
//! install.
mod list;
mod search;

use std::{
  path::PathBuf,
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
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::revision::Revision;
use orchester_workspace::{
  background,
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
  theme::ExtraTypography,
  ui::{
    field::field_error,
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

#[derive(Clone, PartialEq)]
struct Listed(IndexList);
impl DevtoolsInspectable for Listed {}
#[derive(Clone, PartialEq)]
struct Checked(RipgrepCheck);
impl DevtoolsInspectable for Checked {}
type Rebuild = FutureAction<(PathBuf, Revision), (), Message>;

pub(super) struct CodeIndex {
  save: Save,
  rebuild: Rebuild,
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  own: work::OwnSaves,
  list_revision: Signal<u64>,
  check_revision: Signal<u64>,
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
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(ctx, &[Outside::CodeIndex]),
      list_revision: ctx.signal(0),
      check_revision: ctx.signal(0),
      tick: ctx.signal(0),
      watching: Arc::new(AtomicBool::new(false)),
      after_install: Arc::new(AtomicBool::new(false)),
    };
    let save = tab.save.clone();
    let rebuild = tab.rebuild.clone();
    work::follow(
      ctx,
      &save,
      ctx.props::<TabProps>().status.clone(),
      (tab.revision.clone(), tab.stored.clone(), tab.own.clone()),
    );
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
    let state = work::load(
      ctx,
      props.project.open.repo_root.clone(),
      self.revision.get() + self.own.count(&props.project.feed),
      &self.own,
    );
    let list = self.list(ctx, &props);
    let check = self.check(ctx, &props);
    let install = code_index::install_state();
    self.watching.store(
      list.data.as_ref().is_some_and(|list| list.0.building())
        || rebuild.state().get().is_pending()
        || matches!(install, Some(code_index::Install::Running { .. })),
      Ordering::Relaxed,
    );
    if !state.is_pending()
      && let Some(read) = &state.data
    {
      self.stored.set(Some(read.clone()));
    }
    let Some(read) = self.stored.get() else {
      return work::pending(ctx, &state, &self.revision);
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
      group = group
        .rows(list::rows(ctx, &list, &rebuild, &props.project.open.repo_root))
        .rows(search::rows(
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
      state
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
  fn list(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Listed, Message> {
    ctx
      .future(
        (
          props.project.open.repo_root.clone(),
          self.list_revision.get(),
          // The runs an index is used by start and end. A build the tab knows of is followed by its poll; while a
          // run runs and none is, a transcript line may be the runner starting one.
          props
            .project
            .feed
            .run_changes(props.project.feed.running.get() && !self.watching.load(Ordering::Relaxed)),
          props.project.feed.revision.get(),
          props.project.feed.outside.get().of(Outside::CodeIndex),
        ),
        |(root, ..)| {
          tracing::trace!(target: crate::app_log::READ_LOG, read = "code index");
          async move { background::run(move || code_index::list_in(&root).map(Listed)).await? }
        },
      )
      .state()
      .get()
  }
  fn check(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Checked, Message> {
    ctx
      .future(
        (props.project.open.repo_root.clone(), self.check_revision.get()),
        |(root, _)| async move { background::run(move || code_index::check_ripgrep(&root).map(Checked)).await? },
      )
      .state()
      .get()
  }
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
fn rebuild_action(ctx: &mut Ctx) -> Rebuild {
  ctx.future_action(|(root, revision): (PathBuf, Revision)| async move {
    background::run(move || code_index::rebuild_in(&root, &revision)).await?
  })
}
