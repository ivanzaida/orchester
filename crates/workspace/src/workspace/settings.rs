//! The application's settings the window shows (Settings › General, Notifications, Automation's window control) and a
//! project's estimates switch (Project settings › Runs), read and kept in their databases. Every change through these
//! writers is counted in the workspace's [`OutsideChanges`](crate::tasks::outside::OutsideChanges), whoever made it,
//! so each screen that shows the setting reads it again.

use std::path::Path;

use orchester_core::{
  appearance::ThemeSetting,
  limits::ComputerServiceSlots,
  notification::NotificationSettings,
  orchestrator::{ReportSchedule, ReportTime},
  time::Timestamp,
  usage::limits::WarningThreshold,
};
use orchester_store::{ProjectDb, StoreError};

use super::Workspace;
use crate::tasks::outside::{AppOutside, Outside};

impl Workspace {
  /// Whether Orchester estimates the story points of the tasks of the project at `repo_root` that have no person's
  /// value (*Let Orchester estimate story points*, Project settings › Runs): the project's own setting, which
  /// [`Self::open`] copied from the application-wide one where the project had none.
  pub fn estimates_story_points(&self, repo_root: &Path) -> Result<bool, StoreError> {
    ProjectDb::open(repo_root)?.estimates_story_points()
  }

  /// Sets *Let Orchester estimate story points* for the project at `repo_root`; only a person does, in the window.
  pub fn set_estimates_story_points(&self, repo_root: &Path, allowed: bool) -> Result<(), StoreError> {
    let mut db = ProjectDb::open(repo_root)?;
    db.set_estimates_story_points(allowed)?;
    self.changes.record(db.project().id(), Outside::Estimates);
    Ok(())
  }

  /// Where Usage › Plan limits draws its warning line, for every project; 80 % until it is changed.
  pub fn usage_warning_threshold(&self) -> Result<WarningThreshold, StoreError> {
    self.db.lock().usage_warning_threshold()
  }

  pub fn set_usage_warning_threshold(&self, threshold: WarningThreshold) -> Result<(), StoreError> {
    self.db.lock().set_usage_warning_threshold(threshold)?;
    self.changes.record_app(AppOutside::UsageThreshold);
    Ok(())
  }

  /// The computer's *Services at once*, which every project's runner reads at each start of a service; 8 until it is
  /// changed.
  pub fn service_slots(&self) -> Result<ComputerServiceSlots, StoreError> {
    self.db.lock().service_slots()
  }

  /// Keeps the computer's *Services at once*; the runners follow it from their next start.
  pub fn set_service_slots(&self, slots: ComputerServiceSlots) -> Result<(), StoreError> {
    self.db.lock().set_service_slots(slots)
  }

  /// The colors the window uses (`orchester-app`); [`ThemeSetting::default`] until one is chosen.
  pub fn theme(&self) -> Result<ThemeSetting, StoreError> {
    self.db.lock().theme()
  }

  pub fn set_theme(&self, theme: ThemeSetting) -> Result<(), StoreError> {
    self.db.lock().set_theme(theme)
  }

  /// Which OS notifications `orchester-app` shows ([`NotificationSettings::default`] until changed).
  pub fn notification_settings(&self) -> Result<NotificationSettings, StoreError> {
    self.db.lock().notification_settings()
  }

  pub fn set_notification_settings(&self, settings: NotificationSettings) -> Result<(), StoreError> {
    self.db.lock().set_notification_settings(settings)
  }

  /// Whether the app sends its error reports to GlitchTip (`orchester_app::error_reports`); on until a person turns
  /// it off.
  pub fn sends_error_reports(&self) -> Result<bool, StoreError> {
    self.db.lock().sends_error_reports()
  }

  /// Keeps a person's choice of *Send error reports* (Settings › General); the app's client and its runners follow it
  /// at once (`orchester_app::error_reports::turn`), not through this.
  pub fn set_sends_error_reports(&self, allowed: bool) -> Result<(), StoreError> {
    self.db.lock().set_sends_error_reports(allowed)?;
    self.changes.record_app(AppOutside::ErrorReports);
    Ok(())
  }

  /// When the Orchestrator's daily report comes; `None` while it is off, as it is until a person turns it on.
  pub fn daily_report_schedule(&self) -> Result<Option<ReportSchedule>, StoreError> {
    self.db.lock().daily_report_schedule()
  }

  /// Turns the daily report on at `time`, or off with `None`; only the window's setting, which a person changes, calls
  /// it.
  pub fn set_daily_report(&self, time: Option<ReportTime>) -> Result<(), StoreError> {
    self.db.lock().set_daily_report(time, Timestamp::now())?;
    self.changes.record_app(AppOutside::DailyReport);
    Ok(())
  }

  /// Whether agents may control `orchester-app`'s window through its MCP server; `None` until it is
  /// set, when the build decides.
  pub fn mcp_window_control(&self) -> Result<Option<bool>, StoreError> {
    self.db.lock().mcp_window_control()
  }

  /// Keeps a person's choice of window control (*Agents can control this window*), which the app reads when it
  /// starts: a run keeps what it started with, so the choice applies from the next start. Only the window's setting,
  /// which a person changes, calls it.
  pub fn set_mcp_window_control(&self, allowed: bool) -> Result<(), StoreError> {
    self.db.lock().set_mcp_window_control(Some(allowed))?;
    self.changes.record_app(AppOutside::Automation);
    Ok(())
  }
}
