//! *Daily report* (ORC-47, `Daily report` `Uqsxx` in `02 · Notifications`): *Daily report from the Orchestrator* with
//! its switch, off until a person turns it on, and while it is on, *Deliver at* with the time of day the report comes
//! at, in the quiet hours' time select (`super::notifications`). The setting is read once when the group opens and
//! saved off the UI thread at each change; until it is read the switch stays as it is, and a setting that cannot be
//! read or was not saved says why under the row.

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

use super::{
  notifications::{TIME_WIDTH, clock, minute, show, time_options},
  save_state::{Outcome, SaveState},
};
use crate::{
  services::Services,
  ui::{
    field::{SelectField, field_error},
    message::message_text,
    setting_rows::{SettingGroup, SettingRow},
    toggle::{Toggle, ToggleKind},
  },
};

/// The switch's element id, which the row's parts extend.
const ID: &str = "settings-daily-report";

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
}

impl Component for DailyReportSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.signal(None),
      time: ctx.signal(clock(ReportTime::DEFAULT.minute())),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let state = save.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&state));
    }
    let failed = state.error;
    let read = stored(ctx);
    if self.shown.get_untracked().is_none()
      && let Some(Setting(stored)) = read.data
    {
      self.shown.set(Some(Setting(stored)));
      show(&self.time, clock(stored.unwrap_or(ReportTime::DEFAULT).minute()));
    }
    let shown = self.shown.get().map(|Setting(time)| time);
    self.follow_time(shown, &save);
    let title = ctx.t("settings.dailyReport.title");
    let description = ctx.t("settings.dailyReport.description");
    let on = matches!(shown, Some(Some(_)));
    let switch = Toggle::new(ID, ToggleKind::Switch, on, title.clone()).hide_label();
    let switch = match shown {
      Some(_) => self.listen(switch, save),
      None => switch,
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
    let at = on.then(|| self.time_row(ctx));
    SettingGroup::new(format!("{ID}-section"), ctx.t("settings.dailyReport.group"))
      .row(report)
      .rows(at)
  }
}

impl DailyReportSection {
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

/// Saves the time the person chose, or that the report is off.
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

/// The setting as stored, read when the section opens.
fn stored(ctx: &mut Ctx) -> FutureState<Setting, Message> {
  let workspace = Services::of(ctx).workspace;
  ctx
    .future((), move |()| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.daily_report_schedule())
          .await?
          .map(|schedule| Setting(schedule.map(|schedule| schedule.time)))
          .map_err(|error| {
            tracing::error!(%error, "the daily report setting cannot be read");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
}
