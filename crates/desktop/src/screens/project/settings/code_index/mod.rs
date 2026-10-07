//! Project settings / Code index (`i220W` on with its list, `LC2V0` no index yet, `RkMap` off,
//! `h6yMk` ripgrep not found, `NvR7K` install states; `docs/design/orchester-workspace.md`,
//! "Code index"): *Use code index*, which saves when the switch flips, the indexes Orchester keeps
//! with *Rebuild* / *Retry*, and the search tool agents get. While an index builds or ripgrep
//! installs, the tab reads them again every second.

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
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{code_index::CodeIndexSetting, revision::Revision};

use super::{
  SettingsSave,
  general::{CONTENT_WIDTH, readback, sized_setting_row},
  outside::OutsideSettings,
};
use crate::{
  activity::{Activity, OperationSpec, code_index_operations},
  background,
  code_index::{self, IndexList, RipgrepCheck},
  messages::{self, Message},
  theme::ExtraTypography,
  ui::{
    field::{field_error, wrapping_field_error},
    form::form,
    icon::Icon,
    message_text,
    notice::Notice,
    switch::Switch,
  },
  workspace::OpenProject,
};

/// A setting's value column (`EPkdo`).
const VALUE_WIDTH: f32 = 300.0;
/// How often a building index or a running install is read again.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct CodeIndexProps {
  pub open: OpenProject,
  /// Written by the tab, read by the settings topbar.
  pub status: Signal<SettingsSave>,
}

impl PartialEq for CodeIndexProps {
  fn eq(&self, other: &Self) -> bool {
    self.open == other.open
  }
}

impl lurq::app::component::DevtoolsInspectable for CodeIndexProps {}

/// The setting as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stored(CodeIndexSetting);

impl lurq::app::component::DevtoolsInspectable for Stored {}

/// The setting as stored at a reading of [`OutsideSettings`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Read(Stored, u64);

impl lurq::app::component::DevtoolsInspectable for Read {}

type SaveAction = FutureAction<(PathBuf, CodeIndexSetting, OperationSpec), Stored, Message>;
type RebuildAction = FutureAction<(PathBuf, Revision, OperationSpec), (), Message>;

pub struct CodeIndexTab {
  stored: Signal<Option<Stored>>,
  /// Bumped to read the index list again.
  list_revision: Signal<u64>,
  /// Bumped to look for ripgrep again.
  check_revision: Signal<u64>,
  /// Bumped every second while something runs, for elapsed times and install output.
  tick: Signal<u64>,
  /// Something runs that the tab follows: set while rendering, read by the interval.
  watching: Arc<AtomicBool>,
  outside: OutsideSettings,
}

impl Component for CodeIndexTab {
  type Props = CodeIndexProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      stored: ctx.signal(None),
      list_revision: ctx.signal(0),
      check_revision: ctx.signal(0),
      tick: ctx.signal(0),
      watching: Arc::new(AtomicBool::new(false)),
      outside: OutsideSettings::new(ctx),
    };
    let (save, rebuild) = (save_action(ctx), rebuild_action(ctx));
    tab.watch_save(ctx, &save);
    let list = tab.list_revision.clone();
    ctx.watch(&rebuild.state(), move |_| list.update(|revision| *revision += 1));
    let (watching, tick, list, check) = (
      tab.watching.clone(),
      tab.tick.clone(),
      tab.list_revision.clone(),
      tab.check_revision.clone(),
    );
    // Whether an install ran at the last tick: once it ended, ripgrep is looked for again.
    let seen_running = AtomicBool::new(false);
    ctx
      .create_interval(WATCH_INTERVAL, move || {
        let installing = matches!(code_index::install_state(), Some(code_index::Install::Running { .. }));
        if seen_running.swap(installing, Ordering::Relaxed) && !installing {
          check.update(|revision| *revision += 1);
        }
        if watching.load(Ordering::Relaxed) {
          tick.update(|tick| *tick += 1);
          list.update(|revision| *revision += 1);
        }
      })
      .start();
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<CodeIndexProps>().open.clone();
    // Futures are matched by call order: the actions come first, as in `create`.
    let (save, rebuild) = (save_action(ctx), rebuild_action(ctx));
    self.tick.get();
    let setting_error = self.load_setting(ctx, &open, save.state().get().is_pending());
    let list = self.load_list(ctx, &open);
    let check = self.load_check(ctx, &open);
    let install = code_index::install_state();
    let building = matches!(&list, Some(Ok(list)) if list.building()) || rebuild.state().get().is_pending();
    let installing = matches!(install, Some(code_index::Install::Running { .. }));
    self.watching.store(building || installing, Ordering::Relaxed);
    let mut content = Column::new()
      .id("settings-code-index")
      .width(Dimension::Px(CONTENT_WIDTH))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      .child(heading(ctx))
      .with_children(self.outside.notice(ctx, "settings-code-index-changed-outside"));
    let error = setting_error.map(|error| field_error(&message_text(ctx, &error)));
    let Some(Stored(setting)) = self.stored.get() else {
      return form(
        None,
        ScrollVertical::new(content.with_children(error))
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      );
    };
    // A failed reading after a change keeps the setting as last read, and says why.
    content = content
      .with_children(error)
      .child(self.setting_row(ctx, (&open, setting), list.as_ref(), &save));
    content = if setting.is_on() {
      let rebuild_error = rebuild.state().get().error;
      let rebuild = self.rebuild_handler(&open, &rebuild);
      content
        .child(list::panel(ctx, list.as_ref(), &rebuild))
        // Why the last Rebuild or Retry did not replace the index (a failed build's own reason is
        // on its row).
        .with_children(rebuild_error.map(|error| wrapping_field_error(&message_text(ctx, &error))))
        .child(
          // The section space (`vm2Gi`) above the heading.
          Column::new()
            .width(Dimension::Pct(100.0))
            .padding_top(SpacingSize::Lg)
            .child(
              Text::new(&ctx.t("settings.codeIndex.searchHeading")).variant(ExtraTypography::SectionHeading.role()),
            ),
        )
        .with_children(search::section(
          ctx,
          check.as_ref(),
          install.as_ref(),
          (&self.check_revision, &self.tick),
        ))
    } else {
      content.child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_top(SpacingSize::Md)
          .child(
            Notice::new(
              Icon::CircleOff,
              ctx.t("settings.codeIndex.offTitle"),
              ctx.t("settings.codeIndex.offDetail"),
            )
            .id("settings-code-index-off"),
          ),
      )
    };
    form(
      None,
      ScrollVertical::new(content).width(Dimension::Pct(100.0)).flex(1.0),
    )
  }
}

impl CodeIndexTab {
  fn watch_save(&self, ctx: &mut Ctx, save: &SaveAction) {
    let status = ctx.props::<CodeIndexProps>().status.clone();
    let stored = self.stored.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(saved), None) => {
          stored.set(Some(*saved));
          status.set(SettingsSave::Saved);
        }
        (None, None) => {}
      }
    });
  }

  /// Reads the stored setting, again after automation changed it (unless the person's change is
  /// `saving`, see [`OutsideSettings`]); why it cannot be read.
  fn load_setting(&self, ctx: &mut Ctx, open: &OpenProject, saving: bool) -> Option<Message> {
    let deps = (open.repo_root.clone(), self.outside.reading(ctx));
    let setting = ctx.future(deps, |(repo_root, reading)| async move {
      background::run(move || code_index::setting_in(&repo_root))
        .await?
        .map(|setting| Read(Stored(setting), reading))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the code index setting");
          messages::store_error(&error)
        })
    });
    let read = setting.state().get();
    if !read.is_pending()
      && let Some(Read(stored, reading)) = read.data
      && self.outside.take(reading, saving)
      && self.stored.get_untracked() != Some(stored)
    {
      self.stored.set(Some(stored));
    }
    read.error
  }

  fn load_list(&self, ctx: &mut Ctx, open: &OpenProject) -> Option<Result<IndexList, Message>> {
    let deps = (open.repo_root.clone(), self.list_revision.get());
    let list = ctx.future(deps, |(repo_root, _)| async move {
      background::run(move || code_index::list_in(&repo_root)).await?
    });
    let state = list.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Some(Err(error)),
      (Some(list), None) => Some(Ok(list)),
      (None, None) => None,
    }
  }

  fn load_check(&self, ctx: &mut Ctx, open: &OpenProject) -> Option<Result<RipgrepCheck, Message>> {
    let deps = (open.repo_root.clone(), self.check_revision.get());
    let check = ctx.future(deps, |(repo_root, _)| async move {
      background::run(move || code_index::check_ripgrep(&repo_root)).await?
    });
    let state = check.state().get();
    // A check running again keeps showing what the last one found.
    let pending = state.is_pending();
    match (state.data, state.error, pending) {
      (_, Some(error), false) => Some(Err(error)),
      (Some(check), ..) => Some(Ok(check)),
      _ => None,
    }
  }

  /// *Use code index* (`nR378`) with what it means now under the switch.
  fn setting_row(
    &self,
    ctx: &Ctx,
    (open, setting): (&OpenProject, CodeIndexSetting),
    list: Option<&Result<IndexList, Message>>,
    save: &SaveAction,
  ) -> Element {
    let flip = {
      let (save, repo_root) = (save.clone(), open.repo_root.clone());
      let next = if setting.is_on() {
        CodeIndexSetting::Off
      } else {
        CodeIndexSetting::On
      };
      let spec = code_index_operations::code_index_setting(open);
      move || {
        if !save.state().get_untracked().is_pending() {
          save.run((repo_root.clone(), next, spec.clone()));
        }
      }
    };
    let state = ctx.t(if setting.is_on() {
      "settings.codeIndex.on"
    } else {
      "settings.codeIndex.off"
    });
    let value = vec![
      Switch::new("settings-code-index-switch", state, setting.is_on())
        .emphasized()
        .render(flip),
      readback(&list::readback(ctx, setting, list)),
    ];
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("settings.codeIndex.name"),
      &ctx.t("settings.codeIndex.detail"),
      value,
    )
  }

  fn rebuild_handler(&self, open: &OpenProject, rebuild: &RebuildAction) -> list::Rebuild {
    let (rebuild, open) = (rebuild.clone(), open.clone());
    Arc::new(move |revision: Revision| {
      if rebuild.state().get_untracked().is_pending() {
        return;
      }
      let spec = code_index_operations::code_index_rebuild(&open, crate::display::short_revision(revision.as_str()));
      rebuild.run((open.repo_root.clone(), revision, spec));
    })
  }
}

/// *Code index* and what the index gives agents (`Ck3Sn`).
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("settings.codeIndex.heading")).variant(ExtraTypography::SectionHeading.role()))
    .child(Text::new(&ctx.t("settings.codeIndex.explanation")).variant(ExtraTypography::Explanation.role()))
    .into()
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  ctx.future_action(
    move |(repo_root, setting, spec): (PathBuf, CodeIndexSetting, OperationSpec)| {
      let activity = activity.clone();
      async move {
        let work = async move {
          background::run(move || code_index::save_setting_in(&repo_root, setting))
            .await?
            .map(|()| Stored(setting))
            .map_err(|error| {
              tracing::error!(%error, "cannot save the code index setting");
              messages::store_error(&error)
            })
        };
        activity.track(spec, work).await
      }
    },
  )
}

fn rebuild_action(ctx: &mut Ctx) -> RebuildAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, revision, spec): (PathBuf, Revision, OperationSpec)| {
    let activity = activity.clone();
    async move {
      let work = async move { background::run(move || code_index::rebuild_in(&repo_root, &revision)).await? };
      activity.track(spec, work).await
    }
  })
}
