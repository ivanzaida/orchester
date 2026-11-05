//! The application's settings as Settings shows them, one query per stored source, each read through the workspace off
//! the UI thread once reads are released (`ReadHold`, which QA holds to look at the skeletons); and what an account's
//! page shows of it, live.

use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  account::AgentAccount,
  app_update::UpdateSettings,
  connection::plan_limits::{LimitReading, ReadingSource},
  id::AgentAccountId,
  orchestrator::ReportTime,
  time::Timestamp,
  usage::limits::WarningThreshold,
};
use orchester_store::StoreError;
use orchester_workspace::{
  accounts::AccountUses,
  app_paths, background,
  messages::{self, Message},
  workspace::BrowserSettings,
};

use super::Session;
use crate::screens::settings::Consent;

/// What uses an account across the projects (an account's page in Settings › Accounts): live, since runs and
/// connections change it.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn account_uses(session: Session, account: AgentAccountId) -> Result<AccountUses, Message> {
  let workspace = session.0.workspace;
  background::run(move || workspace.agent_account_uses(account))
    .await?
    .map_err(|error| error.message())
}

/// The last reading kept of an account's plan limits, with where it came from (an account's page): live, since runs
/// and reads keep newer ones.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn plan_reading(
  session: Session,
  account: AgentAccountId,
) -> Result<Option<(LimitReading, ReadingSource)>, Message> {
  let workspace = session.0.workspace;
  background::run(move || workspace.last_plan_reading(account))
    .await?
    .map_err(|error| error.message())
}

/// Reads `read` off the UI thread once reads are released, saying `what` in the log when it fails.
async fn stored<T: Send + 'static>(
  Session(services): Session,
  what: &'static str,
  read: impl FnOnce(&crate::services::Services) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, Message> {
  services.hold.released().await;
  background::run(move || read(&services)).await?.map_err(|error| {
    tracing::error!(%error, setting = what, "an application setting cannot be read");
    messages::store_error(&error)
  })
}

/// The share of a plan window at which Usage warns (Settings › General › Plan limits).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn usage_threshold(session: Session) -> Result<WarningThreshold, Message> {
  stored(session, "usage warning threshold", |services| {
    services.workspace.usage_warning_threshold()
  })
  .await
}

/// Whether error reports are sent (Settings › General › Privacy).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn error_reports(session: Session) -> Result<bool, Message> {
  stored(session, "send error reports", |services| {
    services.workspace.sends_error_reports()
  })
  .await
}

/// The update settings and when Orchester Cloud last answered a check (Settings › General › Updates).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredUpdates {
  pub settings: UpdateSettings,
  pub last_checked: Option<Timestamp>,
}

#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn update_settings(session: Session) -> Result<StoredUpdates, Message> {
  stored(session, "update settings", |services| {
    Ok(StoredUpdates {
      settings: services.updates.settings()?,
      last_checked: services.updates.last_checked()?,
    })
  })
  .await
}

/// When the Orchestrator's daily report comes, or `None` while it is off (Settings › Notifications).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn daily_report(session: Session) -> Result<Option<ReportTime>, Message> {
  stored(session, "daily report", |services| {
    Ok(
      services
        .workspace
        .daily_report_schedule()?
        .map(|schedule| schedule.time),
    )
  })
  .await
}

/// Agents' browser as found or chosen, its profiles' folder and the Browser window's switch (Settings › Browser): live,
/// since finding the browser looks at this computer.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn browser_settings(session: Session) -> Result<Arc<BrowserSettings>, Message> {
  stored(session, "browser", |services| {
    services.workspace.browser_settings().map(Arc::new)
  })
  .await
}

/// A consent of what automation may do (Settings › Automation; Project settings › Orchestrator shows one).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn automation_consent(session: Session, consent: Consent) -> Result<bool, Message> {
  stored(session, consent.logged(), move |services| {
    consent.read(&services.workspace)
  })
  .await
}

/// The person's agent CLI accounts (Settings › Accounts), read again as they change.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn agent_accounts(session: Session) -> Result<Arc<Vec<AgentAccount>>, Message> {
  stored(session, "agent accounts", |services| {
    services.workspace.agent_accounts().map(Arc::new)
  })
  .await
}

/// Where the application database and the app's log are (Settings › General › This computer); `None` where one is
/// unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Places {
  pub database: Option<PathBuf>,
  pub log: Option<PathBuf>,
}

#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn places(session: Session) -> Result<Places, Message> {
  stored(session, "places", |services| {
    Ok(Places {
      database: services.workspace.database_path(),
      log: app_paths::log_dir()
        .inspect_err(|error| tracing::warn!(%error, "the application log folder is unknown"))
        .ok(),
    })
  })
  .await
}
