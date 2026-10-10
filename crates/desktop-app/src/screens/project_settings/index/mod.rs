//! Code index (s1K5a, QBdyL): the workspace's setting, revision list, rebuild and ripgrep install.
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
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::revision::Revision;
use orchester_workspace::{
  background,
  code_index::{self, IndexList, RipgrepCheck},
  messages::Message,
  tasks::outside::Outside,
};

use super::{
  TabProps, page,
  work::{self, Change, Read, Save},
};
use crate::ui::{
  field::field_error,
  form_section::{Place, section},
  message::message_text,
  notice::{Notice, NoticeTone},
  toggle::{Toggle, ToggleKind},
};

#[derive(Clone, PartialEq)]
struct Listed(IndexList);
impl DevtoolsInspectable for Listed {}
#[derive(Clone, PartialEq)]
struct Checked(RipgrepCheck);
impl DevtoolsInspectable for Checked {}
type Rebuild = FutureAction<(PathBuf, Revision), (), Message>;

pub(super) struct CodeIndex {
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
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
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      list_revision: ctx.signal(0),
      check_revision: ctx.signal(0),
      tick: ctx.signal(0),
      watching: Arc::new(AtomicBool::new(false)),
      after_install: Arc::new(AtomicBool::new(false)),
    };
    let save = work::action(ctx);
    let rebuild = rebuild_action(ctx);
    work::follow(
      ctx,
      &save,
      ctx.props::<TabProps>().status.clone(),
      tab.revision.clone(),
      tab.stored.clone(),
    );
    let revision = tab.list_revision.clone();
    ctx.watch(&rebuild.state(), move |_| revision.update(|r| *r += 1));
    tab.watch(ctx);
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = work::action(ctx);
    let rebuild = rebuild_action(ctx);
    self.tick.get();
    let state = work::load(
      ctx,
      props.project.open.repo_root.clone(),
      self.revision.get() + props.project.feed.outside.get().of(Outside::CodeIndex),
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
      return page::scroll(work::pending(ctx, &state, &self.revision));
    };
    let on = read.0.code_index.is_on();
    let row = setting(ctx, &props, on, &list, &save);
    let mut rows = vec![
      page::note("settings-index-description", &ctx.t("project_settings.indexDetail")),
      row,
    ];
    if on {
      rows.push(list::panel(ctx, &list, &rebuild, &props.project.open.repo_root));
    } else {
      rows.push(
        Notice::new(
          "settings-index-off",
          NoticeTone::Info,
          ctx.t("project_settings.indexOffTitle"),
        )
        .body(ctx.t("project_settings.indexOffNote"))
        .into(),
      );
    }
    let content = Column::new()
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .with_children(work::error(ctx, &save))
      .with_children(
        state
          .error
          .map(|e| field_error("settings-index-load-error", &message_text(ctx, &e))),
      )
      .child(section(
        "settings-index-section",
        &ctx.t("project_settings.indexHeading"),
        None,
        rows,
      ))
      .with_children(
        rebuild
          .state()
          .get()
          .error
          .map(|e| field_error("settings-index-rebuild-error", &message_text(ctx, &e))),
      )
      .with_children(on.then(|| {
        search::section(
          ctx,
          &check,
          install.as_ref(),
          &self.check_revision,
          &self.tick,
          &self.after_install,
        )
      }));
    page::scroll(content)
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
        (props.project.open.repo_root.clone(), self.list_revision.get()),
        |(root, _)| async move { background::run(move || code_index::list_in(&root).map(Listed)).await? },
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
fn setting(ctx: &Ctx, props: &TabProps, on: bool, list: &FutureState<Listed, Message>, save: &Save) -> Element {
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
  );
  let toggle = if save.state().get().is_pending() {
    toggle
  } else {
    toggle.on_change(move |_| start())
  };
  let readback = list
    .data
    .as_ref()
    .map(|list| ctx.t_args("project_settings.indexKept", [("count", list.0.0.len().to_string())]));
  let control = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(toggle)
    .with_children(readback.map(|text| page::note("settings-index-kept", &text)));
  page::row(
    ctx,
    "settings-index-use",
    ("project_settings.useIndex", "project_settings.indexSettingDetail"),
    control.into(),
    Place::Last,
  )
}
fn rebuild_action(ctx: &mut Ctx) -> Rebuild {
  ctx.future_action(|(root, revision): (PathBuf, Revision)| async move {
    background::run(move || code_index::rebuild_in(&root, &revision)).await?
  })
}
