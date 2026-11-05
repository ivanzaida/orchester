//! *Watching* (`wY6EX` in 07 `DE2Pj`, `docs/design/orchestrator-watch.md` decision 8): how the project's Orchestrator
//! watches its runs, between *The Orchestrator may* and *Also needed*. *Default order for watching* (`cntrS`) is a
//! multi-line field a /watch command without text takes as its order: Shift Enter adds a line, and it saves on Enter
//! or as the person leaves it, like the rest of the tab; empty is no order. *A run without progress for* (`NOCbA`) is
//! whole minutes in a box, *min* and a switch: a wrong value says why under the row and saves nothing, leaving the box
//! puts back the last saved value, and Off fades the box and keeps its minutes for when it is on again (`b00C9G` a–d).
//! Read through its query, which the project's loader read as the project opened, and saved off the UI thread
//! (ORC-757); neither widens what agents may do, so automation may change them as a person can.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::orchestrator::WatchSettings;
use orchester_workspace::{background, messages::Message};
use parking_lot::Mutex;

use super::super::{
  Status, TabProps,
  kit::{Group, SettingRow, on_enter},
};
use crate::{
  queries::{self, Session, project},
  services::Services,
  theme::{ExtraSpacing, ProjectSettingsText},
  ui::{
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
    setting_rows::marked_words,
    skeleton,
    toggle::{Toggle, ToggleKind},
  },
};

/// The group's element id.
const ID: &str = "settings-orchestrator-watching";
/// *Default order for watching*'s row; its field is `<ORDER>-field`.
const ORDER: &str = "settings-orchestrator-default-order";
/// *A run without progress for*'s row; its box is `<NO_PROGRESS>-minutes`, its switch `<NO_PROGRESS>-switch`.
const NO_PROGRESS: &str = "settings-orchestrator-no-progress";
/// The minutes' box (`Minutes` `t9kksP`).
const MINUTES_WIDTH: f32 = 56.0;
/// The bone of the minutes while they are read, as wide as *20*.
const MINUTES_BONE: f32 = 16.0;
/// The most minutes the box takes: whole minutes from 1 to 240 (decision 8, `Error` `E94B6` in `ZnPlx`).
const MOST_MINUTES: u32 = 240;

/// How the Orchestrator watches, as a signal's and a save's value.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Watch(WatchSettings);

impl DevtoolsInspectable for Watch {}

type SaveAction = FutureAction<Watch, Watch, Message>;

#[derive(Clone)]
pub(super) struct Watching {
  order: Signal<String>,
  /// The minutes as typed.
  minutes: Signal<String>,
  /// Whether a run without progress is reported at all.
  on: Signal<bool>,
  /// The minutes last read or saved, which Off keeps and a wrong value goes back to.
  kept: Signal<u32>,
  /// What the fields were filled from or last saved as: the base of the next save; `None` until first read.
  stored: Signal<Option<Watch>>,
  /// A save asked for while another ran.
  queued: Signal<bool>,
  save: SaveAction,
  /// What the query last read, so only a new read fills the fields.
  seen: Arc<Mutex<Option<WatchSettings>>>,
}

impl Component for Watching {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let root = ctx.props::<TabProps>().project.open.repo_root.clone();
    let save = save_action(ctx, root);
    let tab = Self {
      order: ctx.signal(String::new()),
      minutes: ctx.signal(String::new()),
      on: ctx.signal(true),
      kept: ctx.signal(WatchSettings::DEFAULT_NO_PROGRESS_MINUTES),
      stored: ctx.signal(None),
      queued: ctx.signal(false),
      save: save.clone(),
      seen: Arc::default(),
    };
    let after = tab.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if state.error.is_none()
        && let Some(Watch(saved)) = &state.data
      {
        if let Some(minutes) = saved.no_progress_minutes() {
          after.kept.set(minutes);
        }
        after.stored.set(Some(Watch(saved.clone())));
      }
      if after.queued.get_untracked() {
        after.submit();
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(project::orchestrator_watch(
      session,
      props.project.open.repo_root.clone(),
    )));
    let state = self.save.state().get();
    if let Some(data) = &read.data {
      self.follow(data, state.is_pending());
    }
    let stored = self.stored.get();
    let (order, typed, on) = (self.order.get(), self.minutes.get(), self.on.get());
    let invalid = stored.is_some() && on && minutes(&typed).is_none();
    let edited = stored
      .as_ref()
      .is_some_and(|Watch(base)| watch(&order, &typed, on).as_ref() != Some(base));
    props.status.set(match (state.is_pending(), &state.error) {
      (true, _) => Status::Saving,
      (false, Some(_)) => Status::Failed,
      (false, None) if invalid => Status::Invalid,
      (false, None) if edited => Status::Unsaved,
      (false, None) => Status::Saved,
    });
    let rows = vec![
      self.order_row(ctx, stored.is_some()),
      self.no_progress_row(ctx, stored.is_some(), invalid),
    ];
    let place = marked_words(
      &format!("{ID}-where"),
      Icon::Eye,
      &ctx.t("project_settings.orchestrator.watching.where"),
    );
    let foot = ctx.t_args(
      "project_settings.orchestrator.watching.foot",
      [("minutes", WatchSettings::DEFAULT_NO_PROGRESS_MINUTES.to_string())],
    );
    let mut group = Group::new(ID, ctx.t("project_settings.orchestrator.watching.title"))
      .end(place)
      .rows(rows)
      .foot(&foot);
    let problems = [
      (read.error.as_ref(), "unreadable", "project_settings.orchestrator.unreadable"),
      (state.error.as_ref(), "not-saved", "project_settings.orchestrator.notSaved"),
    ];
    for (reason, part, key) in problems {
      if let Some(reason) = reason {
        let reason = message_text(ctx, reason);
        group = group.after(field_error(
          &format!("{ID}-{part}"),
          &ctx.t_args(key, [("reason", &*reason)]),
        ));
      }
    }
    Element::from(group)
  }
}

impl Watching {
  /// *Default order for watching*: its words over the field, which takes the row's width. Until the order is read the
  /// field takes no input.
  fn order_row(&self, ctx: &mut Ctx, read: bool) -> Element {
    let label = ctx.t("project_settings.orchestrator.watching.order.label");
    let field = FieldInput::new(format!("{ORDER}-field"), label.clone(), self.order.clone())
      .compact_area()
      .text_style(ProjectSettingsText::Written.role())
      .placeholder(ctx.t("project_settings.orchestrator.watching.order.placeholder"))
      .foot(ctx.t("project_settings.orchestrator.watching.order.keys"));
    let field = if read {
      let tab = self.clone();
      field.on_leave(move || tab.submit())
    } else {
      field.locked()
    };
    // Enter saves; Shift Enter adds a line.
    let tab = self.clone();
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .on_key_down(on_enter(move || tab.submit()))
      .child(field.render(ctx));
    SettingRow::new(ORDER, label)
      .hint(ctx.t("project_settings.orchestrator.watching.order.hint"))
      .under(control)
      .render(ctx)
  }

  /// *A run without progress for*: the minutes' box, *min* and the switch, and why the minutes are wrong under the row.
  /// Until they are read the box and the switch are bones.
  fn no_progress_row(&self, ctx: &mut Ctx, read: bool, invalid: bool) -> Element {
    let label = ctx.t("project_settings.orchestrator.watching.noProgress.label");
    let on = self.on.get();
    let unit = Text::new(&ctx.t("project_settings.orchestrator.watching.noProgress.unit"))
      .id(format!("{NO_PROGRESS}-unit"))
      .variant(ProjectSettingsText::Value.role())
      .color(if on && read {
        PaletteColor::TextSecondary
      } else {
        PaletteColor::TextMuted
      })
      .nowrap();
    let control = Row::new()
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center);
    let control = if read {
      let field = FieldInput::new(format!("{NO_PROGRESS}-minutes"), label.clone(), self.minutes.clone())
        .width(Dimension::Px(MINUTES_WIDTH))
        .text_style(ProjectSettingsText::MonoValue.role())
        .end_aligned();
      // Off keeps the minutes, faded, for when it is on again.
      let field = if on {
        let tab = self.clone();
        field.on_leave(move || tab.leave_minutes())
      } else {
        field.locked()
      };
      let field = if invalid { field.invalid() } else { field };
      let tab = self.clone();
      let switch = Toggle::new(format!("{NO_PROGRESS}-switch"), ToggleKind::Switch, on, label.clone())
        .hide_label()
        .on_change(move |chosen| {
          tab.on.set(chosen);
          tab.submit();
        });
      control.child(field.render(ctx)).child(unit).child(switch)
    } else {
      control
        .child(skeleton::field(ctx, MINUTES_WIDTH, MINUTES_BONE))
        .child(unit)
        .child(skeleton::switch())
    };
    let problem = invalid.then(|| {
      Element::from(field_error(
        &format!("{NO_PROGRESS}-error"),
        &ctx.t_args(
          "project_settings.orchestrator.watching.noProgress.invalid",
          [
            ("min", WatchSettings::MIN_NO_PROGRESS_MINUTES.to_string()),
            ("max", MOST_MINUTES.to_string()),
          ],
        ),
      ))
    });
    SettingRow::new(NO_PROGRESS, label)
      .hint(ctx.t("project_settings.orchestrator.watching.noProgress.hint"))
      .control(control)
      .under_all(problem)
      .render(ctx)
  }

  /// Fills the fields from what the query read, once it read something new: first, and after a change from elsewhere
  /// or a save, unless a save runs or the fields hold an edit not saved yet.
  fn follow(&self, data: &WatchSettings, saving: bool) {
    {
      let mut seen = self.seen.lock();
      if seen.as_ref() == Some(data) {
        return;
      }
      *seen = Some(data.clone());
    }
    let unedited = self
      .stored
      .get_untracked()
      .is_none_or(|Watch(base)| self.asked().as_ref() == Some(&base));
    if !saving && unedited {
      self.fill(data);
    }
  }

  fn fill(&self, data: &WatchSettings) {
    if let Some(minutes) = data.no_progress_minutes() {
      self.kept.set(minutes);
    }
    self.order.set(data.default_order().to_owned());
    self.minutes.set(self.kept.get_untracked().to_string());
    self.on.set(data.no_progress_minutes().is_some());
    self.stored.set(Some(Watch(data.clone())));
  }

  /// What the fields ask for; `None` while the minutes are wrong.
  fn asked(&self) -> Option<WatchSettings> {
    watch(
      &self.order.get_untracked(),
      &self.minutes.get_untracked(),
      self.on.get_untracked(),
    )
  }

  /// Leaving the box: wrong minutes go back to the last saved ones, and nothing wrong is saved.
  fn leave_minutes(&self) {
    if minutes(&self.minutes.get_untracked()).is_none() {
      self.minutes.set(self.kept.get_untracked().to_string());
    }
    self.submit();
  }

  /// Saves what the fields ask for when it differs from what is stored; a save asked for while another runs follows it.
  fn submit(&self) {
    if self.save.state().get_untracked().is_pending() {
      self.queued.set(true);
      return;
    }
    self.queued.set(false);
    let (Some(Watch(base)), Some(asked)) = (self.stored.get_untracked(), self.asked()) else {
      return;
    };
    if asked != base {
      self.save.run(Watch(asked));
    }
  }
}

/// Whole minutes the box takes, from [`WatchSettings::MIN_NO_PROGRESS_MINUTES`] to [`MOST_MINUTES`].
fn minutes(typed: &str) -> Option<u32> {
  typed
    .trim()
    .parse::<u32>()
    .ok()
    .filter(|minutes| (WatchSettings::MIN_NO_PROGRESS_MINUTES..=MOST_MINUTES).contains(minutes))
}

/// The settings `order`, `typed` minutes and the switch `on` ask for; `None` while the minutes are wrong and on.
fn watch(order: &str, typed: &str, on: bool) -> Option<WatchSettings> {
  let minutes = if on { Some(minutes(typed)?) } else { None };
  WatchSettings::new(order, minutes).ok()
}

/// Saves how the Orchestrator watches the project at `root`; the workspace counts the change, which reads the query
/// again.
fn save_action(ctx: &mut Ctx, root: PathBuf) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |Watch(watch): Watch| {
    let (workspace, root) = (workspace.clone(), root.clone());
    async move {
      let saved = watch.clone();
      background::run(move || workspace.set_orchestrator_watch(&root, &watch))
        .await?
        .map(|()| Watch(saved))
        .inspect_err(|reason| tracing::error!(?reason, "how the Orchestrator watches was not saved"))
    }
  })
}
