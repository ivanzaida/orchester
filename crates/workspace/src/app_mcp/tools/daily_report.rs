//! `get_daily_report`: a project's report of one local day (ORC-47, `crate::orchestrator::report`), the same the
//! project's Orchestrator conversation got for that day when it was delivered, else built now from what the
//! project's database holds; `delivered` says which. Reading only.

use chrono::NaiveDate;
use orchester_core::{time::Timestamp, usage::local_date};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  orchestrator::report,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetDailyReport {
  /// The project's id, or its name.
  pub project: String,
  /// The local day, `2026-10-20`. Without it, today, up to now.
  #[serde(default)]
  pub day: Option<String>,
}

pub async fn get_daily_report(services: &Services, args: GetDailyReport) -> Result<Value, ToolError> {
  let now = Timestamp::now();
  let day = match args.day.as_deref() {
    None => local_date(now),
    Some(text) => NaiveDate::parse_from_str(text, "%Y-%m-%d")
      .map_err(|_| ToolError::InvalidArguments(format!("`day` is a local day such as 2026-10-20, not `{text}`")))?,
  };
  if day > local_date(now) {
    return Err(ToolError::InvalidArguments(format!("`day` {day} has not come yet")));
  }
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let accounts = services.workspace.agent_accounts()?;
    let (read, delivered) = report::report_in(&open.repo_root, accounts, (day, now))
      .map_err(|reason| ToolError::refused("project_data", vec![reason]))?;
    let mut answer = serde_json::to_value(&read).expect("a report is plain data with string keys");
    let fields = answer.as_object_mut().expect("a report is an object");
    fields.insert("project".to_owned(), json!(open.project.id().to_string()));
    fields.insert("delivered".to_owned(), json!(delivered));
    Ok(answer)
  })
  .await
}
