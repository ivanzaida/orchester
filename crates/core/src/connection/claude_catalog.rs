//! The models a Claude Code connection offers. Claude Code has no model command; the only list it
//! gives is the `models` of the answer to the `initialize` control request of its stream-json
//! input (`claude -p --input-format stream-json`), the request the Agent SDK sends for its
//! `supportedModels()`. Checked 2026-09-30 with Claude Code 2.1.89 in a clean agent environment:
//! with `--no-session-persistence` and standard input closed after the request, the CLI answers
//! within about two seconds and exits; it sends no prompt, makes no model call and writes no
//! session (`.tmp/claude-e2e/probe/`).
//!
//! What that list holds depends on the CLI's version: 2.1.89 lists only aliases (`default`,
//! `sonnet`, `sonnet[1m]`, `haiku`, still resolving to the 4.6 generation); 2.1.284 lists aliases
//! and full ids of older models (`opus`, `claude-fable-5-1`, `claude-opus-5`, …), each with its
//! effort levels. `--model` also takes a model's full name (`claude --help`), so the catalog
//! starts with Anthropic's current full ids ([`CURRENT_MODELS`]), then what the CLI listed, then
//! the `opus` and `sonnet` aliases `claude --help` documents. `default` is left out: it names no
//! model but the account's default, which a profile's model cannot express.
//!
//! Efforts: the CLI refuses an `--effort` it does not know before it starts (2.1.89 knows `low`,
//! `medium`, `high` and `max`; 2.1.284 adds `xhigh`). A listed model keeps the levels the CLI gives
//! it. A current id gets the levels its model supports that the CLI accepts: the four 2.1.89
//! documents, and any other the CLI lists for one of its models.
//!
//! The API may still refuse a current model to an old CLI: Opus 5.5 answered Claude Code 2.1.89
//! with "version 2.1.280 or newer is required" (2026-09-30). The run shows that error.

use serde_json::Value;

use super::catalog::CatalogModel;
use crate::resource::ModelId;

/// Arguments that start the CLI to answer one control request on standard input and exit.
pub(super) const MODELS_ARGS: &[&str] = &[
  "-p",
  "--input-format",
  "stream-json",
  "--output-format",
  "stream-json",
  "--verbose",
  "--strict-mcp-config",
  "--setting-sources",
  "",
  "--disable-slash-commands",
  "--no-session-persistence",
  "--tools",
  "",
];
const REQUEST_ID: &str = "orchester-models";
/// The control request whose answer lists the models; the only line on standard input.
pub(super) const MODELS_REQUEST: &str =
  r#"{"type":"control_request","request_id":"orchester-models","request":{"subtype":"initialize"}}"#;

/// Levels Claude Code 2.1.89's `claude --help` documents for `--effort`: what any version accepts.
const DOCUMENTED_EFFORTS: [&str; 4] = ["low", "medium", "high", "max"];
/// Levels the current models support, lowest first.
const MODEL_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
/// Model aliases `claude --help` documents for `--model`.
const DOCUMENTED_ALIASES: [&str; 2] = ["opus", "sonnet"];
/// What the CLI lists that is not a model.
const NOT_A_MODEL: &str = "default";

/// Anthropic's current models by full id, and whether each takes an effort level (Haiku 4.5 does
/// not; the others support [`MODEL_EFFORTS`]).
const CURRENT_MODELS: [(&str, bool); 4] = [
  ("claude-opus-5-5", true),
  ("claude-sonnet-5-5", true),
  ("claude-fable-5-1", true),
  ("claude-haiku-4-5-20251001", false),
];

/// The catalog of a Claude Code connection: the current full ids, then `listed` (the models the
/// CLI listed, when it could), then the documented aliases, each once.
pub(super) fn claude_models(listed: Option<Vec<CatalogModel>>) -> Vec<CatalogModel> {
  let accepted: Vec<&str> = listed
    .iter()
    .flatten()
    .flat_map(CatalogModel::efforts)
    .map(String::as_str)
    .chain(DOCUMENTED_EFFORTS)
    .collect();
  let efforts = |supported: bool, levels: &[&str]| -> Vec<String> {
    levels
      .iter()
      .filter(|level| supported && accepted.contains(level))
      .map(|level| (*level).to_owned())
      .collect()
  };
  let mut models: Vec<CatalogModel> = CURRENT_MODELS
    .into_iter()
    .map(|(id, effort)| CatalogModel::new(model_id(id), efforts(effort, &MODEL_EFFORTS)))
    .collect();
  let aliases: Vec<CatalogModel> = DOCUMENTED_ALIASES
    .into_iter()
    .map(|alias| CatalogModel::new(model_id(alias), efforts(true, &DOCUMENTED_EFFORTS)))
    .collect();
  for model in listed.into_iter().flatten().chain(aliases) {
    if models.iter().all(|known| known.id() != model.id()) {
      models.push(model);
    }
  }
  models
}

fn model_id(id: &str) -> ModelId {
  ModelId::new(id).expect("the built-in model ids are valid")
}

/// The models of the `initialize` answer in the CLI's stream-json output, in its order, without
/// `default`, with the effort levels the CLI gives each; `None` when the output holds no
/// successful answer to [`MODELS_REQUEST`].
pub fn parse_claude_models(output: &str) -> Option<Vec<CatalogModel>> {
  let answer = output
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    .find(|value| {
      value.get("type").and_then(Value::as_str) == Some("control_response")
        && value.pointer("/response/request_id").and_then(Value::as_str) == Some(REQUEST_ID)
    })?;
  if answer.pointer("/response/subtype").and_then(Value::as_str) != Some("success") {
    return None;
  }
  let models = answer.pointer("/response/response/models")?.as_array()?;
  let listed = models
    .iter()
    .filter_map(|model| {
      let value = model.get("value")?.as_str()?;
      if value == NOT_A_MODEL {
        return None;
      }
      let id = ModelId::new(value).ok()?;
      let supports = model.get("supportsEffort").and_then(Value::as_bool).unwrap_or(false);
      let efforts = model
        .get("supportedEffortLevels")
        .and_then(Value::as_array)
        .filter(|_| supports)
        .map(|levels| levels.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
      Some(CatalogModel::new(id, efforts))
    })
    .collect();
  Some(listed)
}
