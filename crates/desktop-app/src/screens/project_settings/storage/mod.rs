//! Runs / Storage (Jsbd1, meNMR): the disk, its guard and the project's retained folders.
mod confirm;
mod disk;
mod read;
mod rows;
mod work;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::workspaces::KeepFree;
use orchester_workspace::{messages::Message, storage::KeptRow, tasks::outside::Outside};

use super::{
  Status, TabProps,
  kit::{Group, SettingRow, back, page_head},
  page,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  services::storage as sampling,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    message::message_text,
  },
};

/// The Storage pane E7GqFt is wider than the other settings panes.
const WIDTH: f32 = 880.0;

pub(super) struct StoragePage {
  keep: Signal<String>,
  stored: Signal<Option<work::SavedKeep>>,
  invalid: Signal<bool>,
  saved_keep: Signal<Option<work::SavedKeep>>,
  refresh: Signal<u64>,
  modal: Signal<bool>,
  save: work::Save,
  prepare: work::Prepare,
  remove: work::Remove,
  failures: Signal<Failures>,
}
#[derive(Clone, Default)]
struct Failures(Vec<(orchester_core::id::RunId, Message)>);
impl lurq::app::component::DevtoolsInspectable for Failures {}

impl Component for StoragePage {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<TabProps>().clone();
    let page = Self {
      keep: ctx.signal(String::new()),
      stored: ctx.signal(None),
      invalid: ctx.signal(false),
      refresh: ctx.signal(0),
      modal: ctx.signal(false),
      saved_keep: ctx.signal(None),
      save: work::save(ctx, props.project.open.clone()),
      prepare: work::prepare(ctx, props.project.open.repo_root.clone()),
      remove: work::remove(ctx, props.project.open.clone()),
      failures: ctx.signal(Failures::default()),
    };
    page.follow_actions(ctx, &props);
    page
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let revision = self.refresh.get();
    let disk = sampling::follow(
      ctx,
      props.project.open.repo_root.clone(),
      revision + props.project.feed.outside.get().of(Outside::ProjectSettings),
    );
    let reading = read::follow(
      ctx,
      props.project.open.repo_root.clone(),
      (
        revision,
        props.project.feed.states.get(),
        props.project.feed.saves.get(),
      ),
    );
    if let Some(Ok(disk)) = &disk {
      self.seed(disk.guard.keep_free);
    }
    let inventory = reading.as_ref().and_then(|r| r.0.as_ref().ok());
    let disk_group = disk::group(ctx, self, &props, disk.as_ref(), inventory);
    let list = rows::list(ctx, self, &props, reading.as_ref());
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb(ctx, &props))
      .child(page_head(
        "storage-head",
        &ctx.t("settings.project.runs.storage.title"),
        Some(&ctx.t("settings.project.runs.storage.lead")),
      ))
      .child(disk_group)
      .child(list);
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(page::column_scroll_wide(body, WIDTH))
      .with_children(confirm::dialog(ctx, self, disk.as_ref().and_then(|r| r.as_ref().ok())))
  }
}

impl StoragePage {
  fn follow_actions(&self, ctx: &mut Ctx, props: &TabProps) {
    let (status, stored, invalid, save) = (
      props.status.clone(),
      self.stored.clone(),
      self.invalid.clone(),
      self.save.clone(),
    );
    ctx.watch(&self.keep, move |value| {
      if (value.is_empty() && stored.get_untracked().is_none()) || save.state().get_untracked().is_pending() {
        return;
      }
      let parsed = value
        .trim()
        .parse::<u64>()
        .map_err(|_| ())
        .and_then(|n| KeepFree::from_gigabytes(n).map_err(|_| ()));
      invalid.set(parsed.is_err());
      status.set(match parsed {
        Err(()) => Status::Invalid,
        Ok(keep) if Some(work::SavedKeep(keep)) == stored.get_untracked() => Status::Saved,
        Ok(_) => Status::Unsaved,
      });
    });
    let (status, refresh, value, stored, saved_keep) = (
      props.status.clone(),
      self.refresh.clone(),
      self.keep.clone(),
      self.stored.clone(),
      self.saved_keep.clone(),
    );
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
      } else if state.error.is_some() {
        status.set(Status::Failed);
      } else if let Some(keep) = state.data {
        status.set(Status::Saved);
        stored.set(Some(keep));
        saved_keep.set(Some(keep));
        value.set(keep.0.gigabytes().to_string());
        refresh.update(|n| *n += 1);
      }
    });
    let modal = self.modal.clone();
    ctx.watch(&self.prepare.state(), move |state| {
      if !state.is_pending() && state.error.is_none() && state.data.is_some() {
        modal.set(true);
      }
    });
    let (refresh, failures) = (self.refresh.clone(), self.failures.clone());
    ctx.watch(&self.remove.state(), move |state| {
      if !state.is_pending()
        && let Some(removed) = &state.data
      {
        failures.set(Failures(removed.0.clone()));
        refresh.update(|n| *n += 1);
      }
    });
  }
  fn seed(&self, keep: KeepFree) {
    let keep = work::SavedKeep(keep);
    if let Some(saved) = self.saved_keep.get() {
      if saved != keep {
        return;
      }
      self.saved_keep.set(None);
    }
    let stored = self.stored.get();
    if stored == Some(keep) {
      return;
    }
    if stored.is_none() || stored.is_some_and(|old| self.keep.get() == old.0.gigabytes().to_string()) {
      self.stored.set(Some(keep));
      self.keep.set(keep.0.gigabytes().to_string());
    }
  }
  fn busy(&self) -> bool {
    self.prepare.state().get().is_pending() || self.remove.state().get().is_pending() || self.modal.get()
  }
  fn ask(&self, runs: Vec<orchester_core::id::RunId>) -> Arc<dyn Fn() + Send + Sync> {
    let prepare = self.prepare.clone();
    Arc::new(move || {
      if !prepare.state().get_untracked().is_pending() {
        prepare.run(runs.clone());
      }
    })
  }
  fn save_keep(&self) -> impl Fn() + Send + Sync + 'static {
    let (value, invalid, save) = (self.keep.clone(), self.invalid.clone(), self.save.clone());
    move || {
      if save.state().get_untracked().is_pending() {
        return;
      }
      let keep = value
        .get_untracked()
        .trim()
        .parse::<u64>()
        .map_err(|_| ())
        .and_then(|n| KeepFree::from_gigabytes(n).map_err(|_| ()));
      invalid.set(keep.is_err());
      if let Ok(keep) = keep {
        save.run(keep);
      }
    }
  }
}

pub fn entry(ctx: &mut Ctx, props: &TabProps) -> Element {
  ctx.mount_keyed::<Entry>("settings-storage-entry", props.clone())
}
struct Entry;
impl Component for Entry {
  type Props = TabProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let disk = sampling::follow(
      ctx,
      props.project.open.repo_root.clone(),
      props.project.feed.outside.get().of(Outside::ProjectSettings),
    );
    let inventory = read::follow(
      ctx,
      props.project.open.repo_root.clone(),
      (0, props.project.feed.states.get(), props.project.feed.saves.get()),
    );
    let kept = inventory
      .as_ref()
      .and_then(|r| r.0.as_ref().ok())
      .and_then(|r| total(&r.rows));
    let hint = match disk.as_ref() {
      Some(Ok(disk)) => ctx.t_args(
        if disk.guard.holds() {
          match disk.paused {
            0 => "settings.project.runs.storage.entryLowWaiting",
            1 => "settings.project.runs.storage.entryLowOne",
            _ => "settings.project.runs.storage.entryLow",
          }
        } else {
          "settings.project.runs.storage.entryCalm"
        },
        [
          ("size", bytes(ctx, kept).to_string()),
          ("free", bytes(ctx, disk.guard.disk.map(|d| d.free)).to_string()),
          ("disk", disk::name(&props.project.open.repo_root)),
          ("count", disk.paused.to_string()),
        ],
      ),
      Some(Err(error)) => message_text(ctx, error),
      None => ctx.t("settings.project.runs.storage.readingDisk"),
    };
    let row = SettingRow::new("settings-storage", ctx.t("settings.project.runs.storage.title"))
      .toned(
        hint,
        if disk.as_ref().is_some_and(|r| r.as_ref().is_ok_and(|d| d.guard.holds())) {
          super::kit::Tone::Warning
        } else {
          super::kit::Tone::Muted
        },
      )
      .control(Text::new(&bytes(ctx, kept)).variant(ExtraTypography::MonoNote.role()))
      .control(crate::ui::icon::icon(
        crate::ui::icon::Icon::ChevronRight,
        crate::theme::IconSize::Md,
        lurq::app::theme::PaletteColor::TextMuted,
      ))
      .opens(Navigation::of(ctx).to(Route::Project(props.project.open.project.id(), ProjectPage::Storage)))
      .render(ctx);
    Group::new("settings-storage-group", ctx.t("settings.project.runs.storage.group")).row(row)
  }
}
fn bytes(ctx: &Ctx, bytes: Option<u64>) -> Arc<str> {
  bytes.map_or_else(
    || ctx.t("settings.project.runs.storage.unknown"),
    |bytes| {
      ctx.t_args(
        "settings.project.runs.storage.gigabytes",
        [(
          "size",
          format!("{:.1}", bytes as f64 / orchester_core::workspaces::GIGABYTE as f64),
        )],
      )
    },
  )
}
fn total(rows: &[KeptRow]) -> Option<u64> {
  rows
    .iter()
    .try_fold(0u64, |sum, row| row.bytes.map(|n| sum.saturating_add(n)))
}
fn error(ctx: &Ctx, id: &str, error: &Message) -> Element {
  field_error(id, &message_text(ctx, error)).into()
}
fn retry(ctx: &Ctx, id: &str, refresh: &Signal<u64>) -> ActionButton {
  let refresh = refresh.clone();
  ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t("common.tryAgain"))
    .compact()
    .on_click(move || refresh.update(|n| *n += 1))
}
fn note(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key)).variant(ExtraTypography::Note.role())
}
fn crumb(ctx: &mut Ctx, props: &TabProps) -> Element {
  let back = back(
    "storage-crumb",
    (
      &ctx.t("project_settings.runs"),
      Some(&ctx.t("settings.project.runs.storage.title")),
    ),
    Navigation::of(ctx).to(Route::Project(
      props.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Runs),
    )),
  );
  if props.status.get() == Status::Saved {
    return back;
  }
  let unavailable = Column::new()
    .opacity(crate::theme::DISABLED_OPACITY)
    .hit_test(lurq::node::HitTestBehavior::None)
    .child(back);
  crate::ui::tooltip::Tooltip::new("storage-crumb-unavailable", ctx.t("project_settings.finishEditing"))
    .wrap(ctx, unavailable)
}
