//! *Daily report* (ORC-47, `Daily report` `Uqsxx` in `02 · Notifications`): *Daily report from the Orchestrator* with
//! its switch, off until a person turns it on, and while it is on, *Deliver at* with the time of day the report comes
//! at, in the quiet hours' time select (`super::notifications`). The setting is read through its query, which the
//! window read as the session started, and saved off the UI thread at each change; until it was ever read the switch
//! and the time are bones (`L09L` `UjuJA`), a change from elsewhere shows in place, and a setting that cannot be read
//! or was not saved says why under the row, a failed save showing the stored setting again.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::orchestrator::ReportTime;
use orchester_workspace::{
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use super::{
  notifications::{TIME_WIDTH, clock, minute, show, time_options},
  save_state::{Outcome, SaveState},
};
use crate::{
  queries::{self, Session, app},
  services::Services,
  ui::{
    field::{SelectField, field_error},
    message::message_text,
    setting_rows::{SettingGroup, SettingRow},
    skeleton::{self, Skeleton},
    toggle::{Toggle, ToggleKind},
  },
};

/// The switch's element id, which the row's parts extend.
const ID: &str = "settings-daily-report";
/// The time's bone in its field while the setting is read (`Text bone` `kpAov`).
const TIME_BONE: f32 = 40.0;

type SaveAction = FutureAction<Option<ReportTime>, (), Message>;

/// The setting: the time the report comes at, or `None` while it is off. A type of its own because lurq's signals
/// need its DevTools marker, which core's `ReportTime` cannot carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Setting(Option<ReportTime>);

impl lurq::app::component::DevtoolsInspectable for Setting {}

pub struct DailyReportSection {
  /// The setting as shown: `None` until it is read.
  shown: Signal<Option<Setting>>,
  /// The time's select, `HH:MM`.
  time: Signal<String>,
  /// The stored setting the section last took, so a change from elsewhere, or a failed save, shows it again.
  seen: Mutex<Option<Setting>>,
}

impl Component for DailyReportSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.signal(None),
      time: ctx.signal(clock(ReportTime::DEFAULT.minute())),
      seen: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let state = save.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&state));
    }
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(app::daily_report(session)));
    self.take(read.data.map(Setting), &state);
    let failed = state.error;
    let shown = self.shown.get().map(|Setting(time)| time);
    self.follow_time(shown, &save);
    let title = ctx.t("settings.dailyReport.title");
    let description = ctx.t("settings.dailyReport.description");
    let on = matches!(shown, Some(Some(_)));
    // Until the setting is read, the switch and the time are bones of their size (`L09L` `UjuJA`), so no state shows
    // that may not be the stored one.
    let reading = shown.is_none() && read.is_pending();
    let switch: Element = if reading {
      Skeleton::new(format!("{ID}-reading"), ctx.t("settings.reading"))
        .inline()
        .show(ctx, skeleton::switch())
    } else {
      let switch = Toggle::new(ID, ToggleKind::Switch, on, title.clone()).hide_label();
      match shown {
        Some(_) => self.listen(switch, save).into(),
        None => switch.into(),
      }
    };
    let problem = match (&read.error, &failed) {
      (Some(reason), _) => Some(("settings.dailyReport.unreadable", reason)),
      (None, Some(reason)) => Some(("settings.dailyReport.notSaved", reason)),
      (None, None) => None,
    }
    .map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      field_error(&format!("{ID}-error"), &ctx.t_args(key, [("reason", &*reason)])).into()
    });
    let report = SettingRow::new(ID, title)
      .hint(ctx.t("settings.dailyReport.hint"))
      .info(description)
      .control(switch)
      .below_all(problem)
      .render(ctx);
    let at = if reading {
      Some(time_reading(ctx))
    } else {
      on.then(|| self.time_row(ctx))
    };
    SettingGroup::new(format!("{ID}-section"), ctx.t("settings.dailyReport.group"))
      .row(report)
      .rows(at)
  }
}

impl DailyReportSection {
  /// Shows the stored setting once it is first read, and again, while no save runs, when it changed elsewhere or a
  /// save failed; what the person chose shows from their choice on.
  fn take(&self, stored: Option<Setting>, state: &FutureState<(), Message>) {
    let Some(stored) = stored else {
      return;
    };
    let moved = self.seen.lock().replace(stored) != Some(stored);
    let shown = self.shown.get_untracked();
    let first = shown.is_none();
    let settled = !state.is_pending() && (moved || state.is_rejected());
    if shown != Some(stored) && (first || settled) {
      self.shown.set(Some(stored));
      show(&self.time, clock(stored.0.unwrap_or(ReportTime::DEFAULT).minute()));
    }
  }

  /// `switch`, turning the report on at the time the select shows, or off, and saving that.
  fn listen(&self, switch: Toggle, save: SaveAction) -> Toggle {
    let (shown, time) = (self.shown.clone(), self.time.clone());
    switch.on_change(move |chosen| {
      let at = chosen.then(|| {
        minute(&time.get_untracked())
          .and_then(ReportTime::new)
          .unwrap_or(ReportTime::DEFAULT)
      });
      shown.set(Some(Setting(at)));
      save.run(at);
    })
  }

  /// Takes the time the person chose in the select while the report is on.
  fn follow_time(&self, shown: Option<Option<ReportTime>>, save: &SaveAction) {
    let Some(Some(stored)) = shown else {
      return;
    };
    let Some(chosen) = minute(&self.time.get()).and_then(ReportTime::new) else {
      return;
    };
    if chosen != stored {
      self.shown.set(Some(Setting(Some(chosen))));
      save.run(Some(chosen));
    }
  }

  /// *Deliver at* and the time's select (`At` `DKuim`).
  fn time_row(&self, ctx: &mut Ctx) -> Element {
    let select = SelectField::new(
      format!("{ID}-time"),
      ctx.t("settings.dailyReport.time"),
      self.time.clone(),
      time_options(),
    )
    .width(Dimension::Px(TIME_WIDTH))
    .render(ctx);
    SettingRow::new(format!("{ID}-at"), ctx.t("settings.dailyReport.at"))
      .control(select)
      .render(ctx)
  }
}

/// *Deliver at* while the setting is read: the time's field with a bone in it (`At` `IPVO6` in `L09L`).
fn time_reading(ctx: &mut Ctx) -> Element {
  let field = skeleton::field(ctx, TIME_WIDTH, TIME_BONE);
  let field = Skeleton::new(format!("{ID}-at-reading"), ctx.t("settings.reading"))
    .inline()
    .show(ctx, field);
  SettingRow::new(format!("{ID}-at"), ctx.t("settings.dailyReport.at"))
    .control(field)
    .render(ctx)
}

/// Saves the time the person chose, or that the report is off; the workspace counts the change, which reads the
/// setting again (`queries`).
fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |time: Option<ReportTime>| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_daily_report(time))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "the daily report setting was not saved");
          messages::store_error(&error)
        })
    }
  })
}
