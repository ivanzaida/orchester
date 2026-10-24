//! Settings of the application rather than of a project: whether automation may answer waiting tool
//! requests through Orchester's MCP server (off until a person turns it on), the warning threshold the
//! Usage dashboard draws in Plan limits (limits belong to accounts, so every project shows the same
//! line; 80 % until it is changed), and `orchester-app`'s theme (dark until changed) and whether agents
//! may control its window through its MCP server (the build decides until it is set), whether
//! automation may widen what agents may do through Orchester's MCP server (off until a person turns it on
//! in the window), the installed app's own MCP server: whether it listens, on which port, and the id of
//! this installation that names its token in the keychain, whether the Orchestrator chat publishes drafts,
//! answers agents, merges runs, merges finished runs on its own, continues interrupted runs and writes with git itself
//! (each off until a person turns it on in the window), which OS notifications the app shows, and when the
//! Orchestrator's daily report comes (off until a person turns it on).

use orchester_core::{
  app_mcp::McpPort,
  appearance::ThemeSetting,
  id::InstallationId,
  notification::{NotificationSettings, QuietHours},
  orchestrator::{ReportSchedule, ReportTime},
  time::Timestamp,
  usage::limits::WarningThreshold,
};
use rusqlite::params;

use super::AppDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  automation_answers_approvals INTEGER NOT NULL DEFAULT 0 CHECK (automation_answers_approvals IN (0, 1))
) STRICT;
INSERT INTO app_settings (id) VALUES (1);
";

pub(super) const THRESHOLD_MIGRATION: &str = r"
-- Usage › Plan limits › Warning threshold (ORC-72), a whole percentage of a window.
ALTER TABLE app_settings ADD COLUMN usage_warning_threshold INTEGER NOT NULL DEFAULT 80
  CHECK (usage_warning_threshold BETWEEN 1 AND 100);
";

pub(super) const APPEARANCE_MIGRATION: &str = r"
-- orchester-app: the window's theme, and window control for its MCP server (NULL: as the build decides).
ALTER TABLE app_settings ADD COLUMN theme TEXT NOT NULL DEFAULT 'dark' CHECK (theme IN ('dark', 'light', 'system'));
ALTER TABLE app_settings ADD COLUMN mcp_window_control INTEGER CHECK (mcp_window_control IN (0, 1));
";

pub(super) const AUTOMATION_PERMISSIONS_MIGRATION: &str = r"
-- Let automation change agent permissions: off until a person turns it on in the window.
ALTER TABLE app_settings ADD COLUMN automation_changes_permissions INTEGER NOT NULL DEFAULT 0
  CHECK (automation_changes_permissions IN (0, 1));
";

pub(super) const APP_MCP_MIGRATION: &str = r"
-- orchester-app's MCP server at a fixed address (docs/design/app-mcp-connection.md): on until turned off, the
-- port set in Settings, and this installation's id, given on first use, which names its token in the keychain.
ALTER TABLE app_settings ADD COLUMN mcp_server_enabled INTEGER NOT NULL DEFAULT 1 CHECK (mcp_server_enabled IN (0, 1));
ALTER TABLE app_settings ADD COLUMN mcp_port INTEGER NOT NULL DEFAULT 47821 CHECK (mcp_port BETWEEN 1024 AND 65535);
ALTER TABLE app_settings ADD COLUMN installation_id TEXT;
";

pub(super) const ORCHESTRATOR_PUBLISHES_MIGRATION: &str = r"
-- Let the Orchestrator publish drafts (docs/design/orchestrator-toolset.md): off until a person turns it on.
ALTER TABLE app_settings ADD COLUMN orchestrator_publishes_drafts INTEGER NOT NULL DEFAULT 0
  CHECK (orchestrator_publishes_drafts IN (0, 1));
";

pub(super) const ORCHESTRATOR_ANSWERS_MERGES_MIGRATION: &str = r"
-- Let the Orchestrator answer agents and Let the Orchestrator merge runs (docs/design/orchestrator-toolset.md,
-- decision 5): each off until a person turns it on.
ALTER TABLE app_settings ADD COLUMN orchestrator_answers_agents INTEGER NOT NULL DEFAULT 0
  CHECK (orchestrator_answers_agents IN (0, 1));
ALTER TABLE app_settings ADD COLUMN orchestrator_merges_runs INTEGER NOT NULL DEFAULT 0
  CHECK (orchestrator_merges_runs IN (0, 1));
";

pub(super) const ORCHESTRATOR_CONTINUES_MIGRATION: &str = r"
-- Let the Orchestrator continue interrupted runs (docs/design/orchestrator-toolset.md, decision 5): off until a
-- person turns it on.
ALTER TABLE app_settings ADD COLUMN orchestrator_continues_interrupted INTEGER NOT NULL DEFAULT 0
  CHECK (orchestrator_continues_interrupted IN (0, 1));
";

pub(super) const ORCHESTRATOR_MERGES_FINISHED_MIGRATION: &str = r"
-- Let the Orchestrator merge finished runs on its own (docs/design/orchestrator-toolset.md, decision 5, ORC-69): off
-- until a person turns it on.
ALTER TABLE app_settings ADD COLUMN orchestrator_merges_finished INTEGER NOT NULL DEFAULT 0
  CHECK (orchestrator_merges_finished IN (0, 1));
";

pub(super) const ORCHESTRATOR_WRITES_GIT_MIGRATION: &str = r"
-- Let the Orchestrator write with git (docs/design/orchestrator-toolset.md, decision 9): off until a person turns it
-- on.
ALTER TABLE app_settings ADD COLUMN orchestrator_writes_git INTEGER NOT NULL DEFAULT 0
  CHECK (orchestrator_writes_git IN (0, 1));
";

pub(super) const NOTIFICATIONS_MIGRATION: &str = r"
-- OS notifications (ORC-56, docs/design/orchester-workspace.md, Notifications): Needs you and Stopped on, Finished
-- and info off, sound on, no quiet hours (both ends NULL) until a person changes them; quiet hours in minutes after
-- local midnight.
ALTER TABLE app_settings ADD COLUMN notify_needs_you INTEGER NOT NULL DEFAULT 1 CHECK (notify_needs_you IN (0, 1));
ALTER TABLE app_settings ADD COLUMN notify_stopped INTEGER NOT NULL DEFAULT 1 CHECK (notify_stopped IN (0, 1));
ALTER TABLE app_settings ADD COLUMN notify_finished INTEGER NOT NULL DEFAULT 0 CHECK (notify_finished IN (0, 1));
ALTER TABLE app_settings ADD COLUMN notify_sound INTEGER NOT NULL DEFAULT 1 CHECK (notify_sound IN (0, 1));
ALTER TABLE app_settings ADD COLUMN quiet_start INTEGER CHECK (quiet_start BETWEEN 0 AND 1439);
ALTER TABLE app_settings ADD COLUMN quiet_end INTEGER CHECK (quiet_end BETWEEN 0 AND 1439);
";

pub(super) const DAILY_REPORT_MIGRATION: &str = r"
-- The Orchestrator's daily report (ORC-47): off (both NULL) until a person turns it on; the time of day it comes at,
-- in minutes after local midnight, and since when it is on, in milliseconds since the Unix epoch.
ALTER TABLE app_settings ADD COLUMN daily_report_minute INTEGER CHECK (daily_report_minute BETWEEN 0 AND 1439);
ALTER TABLE app_settings ADD COLUMN daily_report_since INTEGER;
";

pub(super) const STORY_POINTS_MIGRATION: &str = r"
-- Let Orchester estimate story points (ORC-49): off until a person turns it on (the person's decision, 2026-10-20).
ALTER TABLE app_settings ADD COLUMN estimates_story_points INTEGER NOT NULL DEFAULT 0
  CHECK (estimates_story_points IN (0, 1));
";

impl AppDb {
  /// When the Orchestrator's daily report comes; `None` while it is off, as it is until a person turns it on.
  pub fn daily_report_schedule(&self) -> Result<Option<ReportSchedule>, StoreError> {
    let (minute, since): (Option<u16>, Option<i64>) = self.connection.query_row(
      "SELECT daily_report_minute, daily_report_since FROM app_settings WHERE id = 1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    match (minute, since) {
      (Some(minute), Some(since)) => Ok(Some(ReportSchedule {
        time: ReportTime::new(minute).ok_or_else(|| corrupt("daily report time", minute))?,
        since: Timestamp::from_unix_millis(since),
      })),
      (None, None) => Ok(None),
      (minute, since) => Err(corrupt("daily report", format!("{minute:?} since {since:?}"))),
    }
  }

  /// Turns the daily report on at `time`, or off with `None`; it stays on since when it was turned on (`now` for a
  /// report that was off), so a change of its time delivers no report of a day before.
  pub fn set_daily_report(&mut self, time: Option<ReportTime>, now: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET daily_report_minute = ?1,
         daily_report_since = CASE WHEN ?1 IS NULL THEN NULL ELSE COALESCE(daily_report_since, ?2) END
       WHERE id = 1",
      params![time.map(ReportTime::minute), now.unix_millis()],
    )?;
    Ok(())
  }

  /// Whether Orchester estimates the story points of tasks without a person's value (*Let Orchester estimate story
  /// points*); each estimate asks auto mode's reviewer's model and spends tokens.
  pub fn estimates_story_points(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT estimates_story_points FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_estimates_story_points(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET estimates_story_points = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Which OS notifications the app shows ([`NotificationSettings::default`] until a person changes them).
  pub fn notification_settings(&self) -> Result<NotificationSettings, StoreError> {
    let (needs_you, stopped, finished, sound, start, end): (bool, bool, bool, bool, Option<u16>, Option<u16>) =
      self.connection.query_row(
        "SELECT notify_needs_you, notify_stopped, notify_finished, notify_sound, quiet_start, quiet_end
         FROM app_settings WHERE id = 1",
        [],
        |row| {
          Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
          ))
        },
      )?;
    let quiet = match (start, end) {
      (Some(start), Some(end)) => {
        Some(QuietHours::new(start, end).map_err(|_| corrupt("quiet hours", format!("{start}-{end}")))?)
      }
      (None, None) => None,
      (start, end) => return Err(corrupt("quiet hours", format!("{start:?}-{end:?}"))),
    };
    Ok(NotificationSettings {
      needs_you,
      stopped,
      finished,
      quiet,
      sound,
    })
  }

  pub fn set_notification_settings(&mut self, settings: NotificationSettings) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET notify_needs_you = ?1, notify_stopped = ?2, notify_finished = ?3, notify_sound = ?4,
         quiet_start = ?5, quiet_end = ?6
       WHERE id = 1",
      params![
        settings.needs_you,
        settings.stopped,
        settings.finished,
        settings.sound,
        settings.quiet.map(QuietHours::start),
        settings.quiet.map(QuietHours::end),
      ],
    )?;
    Ok(())
  }
  /// Whether automation may answer waiting tool requests (*Let automation answer approvals*).
  pub fn automation_answers_approvals(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT automation_answers_approvals FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_automation_answers_approvals(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET automation_answers_approvals = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Where Plan limits draws its dashed warning line.
  pub fn usage_warning_threshold(&self) -> Result<WarningThreshold, StoreError> {
    let percent: i64 = self.connection.query_row(
      "SELECT usage_warning_threshold FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?;
    u32::try_from(percent)
      .ok()
      .and_then(|percent| WarningThreshold::new(percent).ok())
      .ok_or_else(|| corrupt("usage warning threshold", percent))
  }

  pub fn set_usage_warning_threshold(&mut self, threshold: WarningThreshold) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET usage_warning_threshold = ?1 WHERE id = 1",
      params![threshold.percent()],
    )?;
    Ok(())
  }

  /// The theme `orchester-app` shows.
  pub fn theme(&self) -> Result<ThemeSetting, StoreError> {
    let name: String = self
      .connection
      .query_row("SELECT theme FROM app_settings WHERE id = 1", [], |row| row.get(0))?;
    ThemeSetting::from_name(&name).ok_or_else(|| corrupt("theme", name))
  }

  pub fn set_theme(&mut self, theme: ThemeSetting) -> Result<(), StoreError> {
    self
      .connection
      .execute("UPDATE app_settings SET theme = ?1 WHERE id = 1", params![theme.name()])?;
    Ok(())
  }

  /// Whether agents may control `orchester-app`'s window through its MCP server; `None` until it is set,
  /// when the build decides.
  pub fn mcp_window_control(&self) -> Result<Option<bool>, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT mcp_window_control FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?,
    )
  }

  pub fn set_mcp_window_control(&mut self, allowed: Option<bool>) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET mcp_window_control = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether automation may widen what agents may do through Orchester's MCP server (*Let automation change
  /// agent permissions*): broader profile permissions, a CLI program of its choice, environment variables
  /// that change what programs load. Narrowing never needs it.
  pub fn automation_changes_permissions(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT automation_changes_permissions FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_automation_changes_permissions(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET automation_changes_permissions = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the Orchestrator chat publishes the drafts it saved itself (*Let the Orchestrator publish drafts*)
  /// instead of proposing each publication to the person. What widens agents' permissions still needs
  /// [`Self::automation_changes_permissions`].
  pub fn orchestrator_publishes_drafts(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT orchestrator_publishes_drafts FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_publishes_drafts(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET orchestrator_publishes_drafts = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the Orchestrator chat answers agents' questions and sends a run's agents messages itself (*Let the
  /// Orchestrator answer agents*) instead of proposing each to the person; a wait of an agent then also starts an
  /// Orchestrator turn by itself.
  pub fn orchestrator_answers_agents(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT orchestrator_answers_agents FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_answers_agents(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET orchestrator_answers_agents = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the Orchestrator chat's `merge_run` merges at once (*Let the Orchestrator merge runs*) instead of
  /// proposing the merge to the person; `merge_run`'s own rules hold either way.
  pub fn orchestrator_merges_runs(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT orchestrator_merges_runs FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_merges_runs(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET orchestrator_merges_runs = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the Orchestrator chat's `resume_run` continues a run a crash, the runner going away or *Quit now*
  /// interrupted (*Let the Orchestrator continue interrupted runs*); a run stopped at a limit or by a person stays
  /// the person's either way.
  pub fn orchestrator_continues_interrupted(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT orchestrator_continues_interrupted FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_continues_interrupted(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET orchestrator_continues_interrupted = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the Orchestrator chat's git tools write in the project's repository (*Let the Orchestrator write with
  /// git*): commit, branch, switch, pull, push, merge, revert, tag and fetch; what can lose work is proposed either
  /// way.
  pub fn orchestrator_writes_git(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT orchestrator_writes_git FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_writes_git(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET orchestrator_writes_git = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the turn a run's success starts by itself merges the run (*Let the Orchestrator merge finished runs on
  /// its own*), with *Let the Orchestrator merge runs* and *answer agents* on too and the run passing the chat's rule
  /// for it; off, that turn only tells the person.
  pub fn orchestrator_merges_finished(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT orchestrator_merges_finished FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_merges_finished(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET orchestrator_merges_finished = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether the installed app's MCP server listens (*Let agents connect to Orchester*); on until turned off.
  pub fn mcp_server_enabled(&self) -> Result<bool, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT mcp_server_enabled FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?,
    )
  }

  pub fn set_mcp_server_enabled(&mut self, enabled: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET mcp_server_enabled = ?1 WHERE id = 1",
      params![enabled],
    )?;
    Ok(())
  }

  /// The port the installed app's MCP server listens on; [`McpPort::DEFAULT`] until it is changed.
  pub fn mcp_port(&self) -> Result<McpPort, StoreError> {
    let port: i64 = self
      .connection
      .query_row("SELECT mcp_port FROM app_settings WHERE id = 1", [], |row| row.get(0))?;
    u32::try_from(port)
      .ok()
      .and_then(|port| McpPort::new(port).ok())
      .ok_or_else(|| corrupt("MCP port", port))
  }

  pub fn set_mcp_port(&mut self, port: McpPort) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET mcp_port = ?1 WHERE id = 1",
      params![port.get()],
    )?;
    Ok(())
  }

  /// This installation's id, given the first time it is asked for and the same from then on.
  pub fn installation_id(&mut self) -> Result<InstallationId, StoreError> {
    let given = InstallationId::generate().to_string();
    // Only where none is yet: two callers at once keep the first id.
    self.connection.execute(
      "UPDATE app_settings SET installation_id = ?1 WHERE id = 1 AND installation_id IS NULL",
      params![given],
    )?;
    let id: String = self
      .connection
      .query_row("SELECT installation_id FROM app_settings WHERE id = 1", [], |row| {
        row.get(0)
      })?;
    parse("installation id", &id)
  }
}
