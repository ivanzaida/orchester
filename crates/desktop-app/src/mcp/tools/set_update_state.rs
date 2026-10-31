//! `set_update_state`, in debug builds only: shows any update state `update_state` reports in place of this
//! copy's real one, so QA sees each row of Settings › Updates (`c69Eti`), the status bar's update item (S1–S6), the
//! install question (`MGILm`) and the ready notification on screen (`AGENTS.md` rule 15). It takes what
//! `update_state` answers, and `real: true` shows the real state again. Nothing is checked, downloaded or installed
//! for a state set here, nothing of it is written to a database, and it is gone at the next start
//! (`AppUpdates::set_for_testing`).

use std::path::PathBuf;

use lurq::mcp::{McpTool, Scope};
use orchester_api::releases::ReleaseVersion;
use orchester_core::time::Timestamp;
use orchester_workspace::app_updates::{
  AppUpdates, CopyReason, NewestRelease, ReadyUpdate, UpdateFailure, UpdateState,
  install::{InstallFailure, InstallStep, JustUpdated},
};
use serde_json::{Map, Value, json};

use super::{
  ToolError, object, reply,
  update_state::{failure_name, reason_name, update_state},
};

const DESCRIPTION: &str = "Debug builds only. Shows an update state in place of this copy's real one, so Settings › \
  Updates, the status bar's update item, the install question and the ready notification can be seen in each \
  state. Takes what update_state answers: state with its fields (up_to_date checked_at; downloading or paused \
  version, received, total; ready version, notes, critical; waiting_for_runs the same and runs; installing the same \
  and step, with runs while it is waiting_for_runs; failed version and reason; install_failed version, reason, \
  exit_code, log; cloud_unreachable last_checked; not_this_copy reason and newest; not_on_this_platform newest) and \
  updated (version and notes of the update this start finished, for Updated to, or null); running and percent are \
  ignored. `real: true` shows the real state again. Nothing is checked, downloaded, saved or installed for a state \
  set here: Restart to update and Install now say it was set for testing. It is gone at the next start. Answers \
  what update_state answers, with set_for_testing.";

/// The states by the names `update_state` gives them.
const STATES: [&str; 12] = [
  "up_to_date",
  "checking",
  "downloading",
  "paused",
  "ready",
  "waiting_for_runs",
  "installing",
  "failed",
  "install_failed",
  "cloud_unreachable",
  "not_this_copy",
  "not_on_this_platform",
];
/// An install's steps by the names `update_state` gives them.
const STEPS: [&str; 4] = ["waiting_for_runs", "stopping_runs", "backing_up", "handing_over"];
const FAILURES: [UpdateFailure; 6] = [
  UpdateFailure::NoInstaller,
  UpdateFailure::WrongSize,
  UpdateFailure::WrongDigest,
  UpdateFailure::UnknownKey,
  UpdateFailure::BadSignature,
  UpdateFailure::NotSaved,
];
const REASONS: [CopyReason; 5] = [
  CopyReason::Development,
  CopyReason::Qa,
  CopyReason::ScriptInstall,
  CopyReason::Portable,
  CopyReason::ProgramFolderUnknown,
];

pub fn tool(updates: AppUpdates) -> McpTool {
  McpTool::new("set_update_state")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(schema())
    .handler(move |_ctx, args| reply(set_update_state(&updates, &args)))
}

fn schema() -> Value {
  let text = json!({ "type": "string" });
  let millis = json!({ "type": ["integer", "null"] });
  json!({
    "type": "object",
    "properties": {
      "real": { "type": "boolean" },
      "state": { "type": "string", "enum": STATES },
      "version": text,
      "received": { "type": "integer", "minimum": 0 },
      "total": { "type": "integer", "minimum": 0 },
      "notes": text,
      "critical": { "type": "boolean" },
      "runs": { "type": "array", "items": text },
      "step": { "type": "string", "enum": STEPS },
      "reason": text,
      "exit_code": { "type": ["integer", "null"] },
      "log": { "type": ["string", "null"] },
      "checked_at": millis,
      "last_checked": millis,
      "newest": { "type": ["object", "null"], "properties": { "version": text, "download": text } },
      "updated": { "type": ["object", "null"], "properties": { "version": text, "notes": text } },
    },
  })
}

/// Shows the state `args` name, or the real one again with `real: true`, and answers what `update_state` answers.
pub fn set_update_state(updates: &AppUpdates, args: &Value) -> Result<Value, ToolError> {
  let args = object(args)?;
  match args.get("real") {
    Some(Value::Bool(true)) => updates.show_real_state(),
    None | Some(Value::Bool(false)) => updates.set_for_testing(state_of(&args)?, updated(&args)?),
    Some(_) => return Err(ToolError::NotABool { param: "real" }),
  }
  let mut answer = update_state(updates);
  answer["set_for_testing"] = json!(updates.is_set_for_testing());
  Ok(answer)
}

fn state_of(args: &Map<String, Value>) -> Result<UpdateState, ToolError> {
  let name = text(args, "state")?;
  Ok(match name {
    "up_to_date" => UpdateState::UpToDate {
      checked_at: time(args, "checked_at")?.unwrap_or_else(Timestamp::now),
    },
    "checking" => UpdateState::Checking,
    "downloading" | "paused" => {
      let (version, received, total) = (
        version(args, "version")?,
        bytes(args, "received")?,
        bytes(args, "total")?,
      );
      if name == "paused" {
        UpdateState::Paused {
          version,
          received,
          total,
        }
      } else {
        UpdateState::Downloading {
          version,
          received,
          total,
        }
      }
    }
    "ready" => UpdateState::Ready(ready(args)?),
    "waiting_for_runs" => UpdateState::WaitingForRuns {
      update: ready(args)?,
      runs: runs(args)?,
    },
    "installing" => UpdateState::Installing {
      update: ready(args)?,
      step: step(args)?,
    },
    "failed" => UpdateState::Failed {
      version: version(args, "version")?,
      failure: named(args, "reason", &FAILURES, failure_name)?,
    },
    "install_failed" => UpdateState::InstallFailed {
      version: version(args, "version")?,
      failure: install_failure(args)?,
    },
    "cloud_unreachable" => UpdateState::CloudUnreachable {
      last_checked: time(args, "last_checked")?,
    },
    "not_this_copy" => UpdateState::NotThisCopy {
      reason: named(args, "reason", &REASONS, reason_name)?,
      newest: newest(args)?,
    },
    "not_on_this_platform" => UpdateState::NotOnThisPlatform { newest: newest(args)? },
    other => {
      return Err(invalid(
        "state",
        format!("unknown state `{other}`; one of {}", STATES.join(", ")),
      ));
    }
  })
}

/// The update a ready, waiting or installing state shows. It has no installer: nothing installs from a state set for
/// testing.
fn ready(args: &Map<String, Value>) -> Result<ReadyUpdate, ToolError> {
  Ok(ReadyUpdate {
    version: version(args, "version")?,
    notes: optional_text(args, "notes")?.unwrap_or_default().to_owned(),
    critical: flag(args, "critical")?,
    installer: PathBuf::new(),
  })
}

fn step(args: &Map<String, Value>) -> Result<InstallStep, ToolError> {
  Ok(match text(args, "step")? {
    "waiting_for_runs" => InstallStep::WaitingForRuns(runs(args)?),
    "stopping_runs" => InstallStep::StoppingRuns,
    "backing_up" => InstallStep::BackingUp,
    "handing_over" => InstallStep::HandingOver,
    other => {
      return Err(invalid(
        "step",
        format!("unknown step `{other}`; one of {}", STEPS.join(", ")),
      ));
    }
  })
}

fn install_failure(args: &Map<String, Value>) -> Result<InstallFailure, ToolError> {
  let exit_code = match args.get("exit_code") {
    None | Some(Value::Null) => None,
    Some(code) => Some(
      code
        .as_i64()
        .and_then(|code| i32::try_from(code).ok())
        .ok_or_else(|| invalid("exit_code", "must be a whole number or null"))?,
    ),
  };
  Ok(InstallFailure {
    reason: text(args, "reason")?.to_owned(),
    exit_code,
    log: optional_text(args, "log")?.map(PathBuf::from),
  })
}

fn newest(args: &Map<String, Value>) -> Result<Option<NewestRelease>, ToolError> {
  let Some(newest) = release(args, "newest")? else {
    return Ok(None);
  };
  Ok(Some(NewestRelease {
    version: version(newest, "version")?,
    download: text(newest, "download")?.to_owned(),
  }))
}

/// `updated`: the update shown as finished at this start (the status bar's S6 and What's new).
fn updated(args: &Map<String, Value>) -> Result<Option<JustUpdated>, ToolError> {
  let Some(updated) = release(args, "updated")? else {
    return Ok(None);
  };
  Ok(Some(JustUpdated {
    version: version(updated, "version")?,
    notes: optional_text(updated, "notes")?.unwrap_or_default().to_owned(),
  }))
}

/// The object `param`, or none for null.
fn release<'a>(args: &'a Map<String, Value>, param: &'static str) -> Result<Option<&'a Map<String, Value>>, ToolError> {
  match args.get(param) {
    None | Some(Value::Null) => Ok(None),
    Some(Value::Object(release)) => Ok(Some(release)),
    Some(_) => Err(invalid(param, "must be an object with version or null")),
  }
}

fn runs(args: &Map<String, Value>) -> Result<Vec<String>, ToolError> {
  match args.get("runs") {
    None | Some(Value::Null) => Err(ToolError::Missing { param: "runs" }),
    Some(Value::Array(runs)) => runs
      .iter()
      .map(|run| {
        run
          .as_str()
          .map(str::to_owned)
          .ok_or_else(|| invalid("runs", "must name each run by its task key"))
      })
      .collect(),
    Some(_) => Err(invalid("runs", "must be a list of task keys")),
  }
}

/// One of `all` by the name `name` gives it.
fn named<T: Copy>(
  args: &Map<String, Value>,
  param: &'static str,
  all: &[T],
  name: fn(T) -> &'static str,
) -> Result<T, ToolError> {
  let given = text(args, param)?;
  all.iter().copied().find(|item| name(*item) == given).ok_or_else(|| {
    let known: Vec<&str> = all.iter().map(|item| name(*item)).collect();
    invalid(param, format!("unknown `{given}`; one of {}", known.join(", ")))
  })
}

fn version(args: &Map<String, Value>, param: &'static str) -> Result<ReleaseVersion, ToolError> {
  ReleaseVersion::parse(text(args, param)?).map_err(|error| invalid(param, error.to_string()))
}

fn bytes(args: &Map<String, Value>, param: &'static str) -> Result<u64, ToolError> {
  match args.get(param) {
    None | Some(Value::Null) => Err(ToolError::Missing { param }),
    Some(count) => count
      .as_u64()
      .ok_or_else(|| invalid(param, "must be a whole number of bytes")),
  }
}

/// Milliseconds since the Unix epoch, or none for null.
fn time(args: &Map<String, Value>, param: &'static str) -> Result<Option<Timestamp>, ToolError> {
  match args.get(param) {
    None | Some(Value::Null) => Ok(None),
    Some(millis) => millis
      .as_i64()
      .map(|millis| Some(Timestamp::from_unix_millis(millis)))
      .ok_or_else(|| invalid(param, "must be milliseconds since the Unix epoch or null")),
  }
}

fn flag(args: &Map<String, Value>, param: &'static str) -> Result<bool, ToolError> {
  match args.get(param) {
    None | Some(Value::Null) => Ok(false),
    Some(Value::Bool(flag)) => Ok(*flag),
    Some(_) => Err(ToolError::NotABool { param }),
  }
}

fn text<'a>(args: &'a Map<String, Value>, param: &'static str) -> Result<&'a str, ToolError> {
  optional_text(args, param)?.ok_or(ToolError::Missing { param })
}

fn optional_text<'a>(args: &'a Map<String, Value>, param: &'static str) -> Result<Option<&'a str>, ToolError> {
  match args.get(param) {
    None | Some(Value::Null) => Ok(None),
    Some(Value::String(text)) => Ok(Some(text)),
    Some(_) => Err(ToolError::NotAString {
      param: param.to_owned(),
    }),
  }
}

fn invalid(param: &'static str, reason: impl Into<String>) -> ToolError {
  ToolError::InvalidParam {
    param,
    reason: reason.into(),
  }
}
