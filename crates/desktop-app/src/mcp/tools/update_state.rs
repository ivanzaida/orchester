use lurq::mcp::{McpTool, Scope};
use orchester_workspace::app_updates::{
  AppUpdates, CopyReason, NewestRelease, ReadyUpdate, UpdateFailure, UpdateState,
};
use serde_json::{Value, json};

const DESCRIPTION: &str = "Where this copy's own update stands, as Settings › Updates shows it: `running` (this copy's \
  version) and `state`: up_to_date (checked_at), checking, downloading (version, received, total, percent), ready \
  (version, notes, critical), waiting_for_runs (the same and runs), failed (version, reason: no_installer, wrong_size, \
  wrong_digest, unknown_key, bad_signature, not_saved), cloud_unreachable (last_checked), not_this_copy (reason: \
  development, qa, script_install, portable, program_folder_unknown; newest with its download, or null) or \
  not_on_this_platform (newest). Times are milliseconds since the Unix epoch. Read only: no tool installs an update.";

pub fn tool(updates: AppUpdates) -> McpTool {
  McpTool::new("update_state")
    .description(DESCRIPTION)
    .input_schema(json!({ "type": "object", "properties": {}, "additionalProperties": false }))
    .scope(Scope::Observe)
    .read_only()
    .async_handler(move |_| {
      let updates = updates.clone();
      async move { Ok(update_state(&updates)) }
    })
}

/// What the tool answers for `updates`.
pub fn update_state(updates: &AppUpdates) -> Value {
  let mut answer = state_json(&updates.state());
  answer["running"] = json!(updates.this_app().version.as_str());
  answer
}

fn state_json(state: &UpdateState) -> Value {
  match state {
    UpdateState::UpToDate { checked_at } => json!({ "state": "up_to_date", "checked_at": checked_at.unix_millis() }),
    UpdateState::Checking => json!({ "state": "checking" }),
    UpdateState::Downloading {
      version,
      received,
      total,
    } => json!({
      "state": "downloading",
      "version": version.as_str(),
      "received": received,
      "total": total,
      "percent": state.percent(),
    }),
    UpdateState::Ready(update) => ready_json("ready", update),
    UpdateState::WaitingForRuns { update, runs } => {
      let mut answer = ready_json("waiting_for_runs", update);
      answer["runs"] = json!(runs);
      answer
    }
    UpdateState::Failed { version, failure } => json!({
      "state": "failed",
      "version": version.as_str(),
      "reason": failure_name(*failure),
    }),
    UpdateState::CloudUnreachable { last_checked } => json!({
      "state": "cloud_unreachable",
      "last_checked": last_checked.map(|at| at.unix_millis()),
    }),
    UpdateState::NotThisCopy { reason, newest } => json!({
      "state": "not_this_copy",
      "reason": reason_name(*reason),
      "newest": newest.as_ref().map(newest_json),
    }),
    UpdateState::NotOnThisPlatform { newest } => json!({
      "state": "not_on_this_platform",
      "newest": newest.as_ref().map(newest_json),
    }),
  }
}

fn ready_json(state: &str, update: &ReadyUpdate) -> Value {
  json!({
    "state": state,
    "version": update.version.as_str(),
    "notes": update.notes,
    "critical": update.critical,
  })
}

fn newest_json(newest: &NewestRelease) -> Value {
  json!({ "version": newest.version.as_str(), "download": newest.download })
}

const fn failure_name(failure: UpdateFailure) -> &'static str {
  match failure {
    UpdateFailure::NoInstaller => "no_installer",
    UpdateFailure::WrongSize => "wrong_size",
    UpdateFailure::WrongDigest => "wrong_digest",
    UpdateFailure::UnknownKey => "unknown_key",
    UpdateFailure::BadSignature => "bad_signature",
    UpdateFailure::NotSaved => "not_saved",
  }
}

const fn reason_name(reason: CopyReason) -> &'static str {
  match reason {
    CopyReason::Development => "development",
    CopyReason::Qa => "qa",
    CopyReason::ScriptInstall => "script_install",
    CopyReason::Portable => "portable",
    CopyReason::ProgramFolderUnknown => "program_folder_unknown",
  }
}
