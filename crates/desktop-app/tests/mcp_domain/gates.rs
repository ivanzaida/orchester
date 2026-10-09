//! What automation may not decide alone: widening what agents may do is refused as `person_only` while
//! *Let automation change agent permissions* is off, and taken once a person turned it on; narrowing is
//! always taken. Also the pipeline tools' narrowing rules, and the feed hearing of pipeline changes.

use std::path::Path;

use serde_json::{Value, json};

use super::Served;

/// Kontur seeded empty, its Codex connection's program any file the seed records as checked.
fn served() -> Served {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  Served::seeded_with("empty", Some(codex)).0
}

impl Served {
  /// A person turns *Let automation change agent permissions* on or off in the window.
  pub(super) fn let_automation_widen(&self, allowed: bool) {
    self
      .services()
      .workspace
      .set_automation_changes_permissions(allowed)
      .expect("the setting is saved");
  }

  /// The permissions a new profile gets when none are given: the most restrictive its CLI enforces.
  fn most_restrictive(&mut self) -> Value {
    let probe = self.ok("save_profile_draft", &profile("Probe", None));
    probe["profile"]["permissions"].clone()
  }
}

fn profile(role: &str, permissions: Option<Value>) -> Value {
  let mut args = json!({ "project": "Kontur", "role": role, "connection": "Codex · Kontur", "model": "gpt-5.5" });
  if let Some(permissions) = permissions {
    args["permissions"] = permissions;
  }
  args
}

fn permissions(base: &Value, repository: &str, commands: &str) -> Value {
  json!({ "repository": repository, "commands": commands, "network": base["network"] })
}

fn assert_person_only(error: &Value) {
  assert_eq!(error["code"], "person_only", "{error}");
  assert!(
    error["reasons"].as_array().is_some_and(|reasons| reasons.len() > 1),
    "{error}"
  );
}

#[test]
fn domain_widening_a_profile_needs_a_person_and_narrowing_never_does() {
  let mut served = served();
  let base = served.most_restrictive();
  let wide = permissions(&base, "write", "any_in_worktree");
  assert_person_only(&served.refused("save_profile_draft", &profile("Builder 2", Some(wide.clone()))));
  served.let_automation_widen(true);
  served.ok("save_profile_draft", &profile("Builder 2", Some(wide.clone())));
  served.ok(
    "publish_profile",
    &json!({ "project": "Kontur", "profile": "Builder 2" }),
  );
  served.let_automation_widen(false);
  let mut narrow = profile("Builder 2", Some(permissions(&base, "read_only", "ask_before")));
  narrow["profile"] = json!("Builder 2");
  served.ok("save_profile_draft", &narrow);
  let mut same = profile("Builder 2", Some(wide));
  same["profile"] = json!("Builder 2");
  served.ok("save_profile_draft", &same);
  let mut automatic = permissions(&base, "write", "any_in_worktree");
  automatic["approve_automatically"] = json!(true);
  let mut auto = profile("Builder 2", Some(automatic));
  auto["profile"] = json!("Builder 2");
  assert_person_only(&served.refused("save_profile_draft", &auto));
  served.let_automation_widen(true);
  served.ok("save_profile_draft", &auto);
  served.let_automation_widen(false);
  assert_person_only(&served.refused(
    "publish_profile",
    &json!({ "project": "Kontur", "profile": "Builder 2" }),
  ));
  served.let_automation_widen(true);
  served.ok(
    "publish_profile",
    &json!({ "project": "Kontur", "profile": "Builder 2" }),
  );
  served.let_automation_widen(false);
  auto["responsibilities"] = json!("Implements and approves its own requests.");
  served.ok("save_profile_draft", &auto);
}

#[test]
fn domain_a_cli_of_automations_choosing_needs_a_person_before_it_runs() {
  let mut served = served();
  let program = served.folder.path().join("my-codex.exe");
  let args = json!({ "project": "Kontur", "kind": "codex", "name": "Mine", "program": program.display().to_string() });
  assert_person_only(&served.refused("add_cli_connection", &args));
  served.let_automation_widen(true);
  assert_eq!(
    served.refused("add_cli_connection", &args)["code"],
    "not_verified",
    "checked once a person lets it"
  );
}

#[test]
fn domain_only_harmless_variables_change_without_a_person() {
  let mut served = served();
  let set = |variables: Value| json!({ "project": "Kontur", "variables": variables });
  let path = json!([{ "name": "RUST_LOG", "value": "info" }, { "name": "Path", "value": "C:\tools" }]);
  assert_person_only(&served.refused("set_project_environment", &set(path.clone())));
  for name in ["OPENAI_BASE_URL", "CLAUDE_CODE_SHELL_PREFIX", "DATABASE_NAME"] {
    assert_person_only(&served.refused("set_project_environment", &set(json!([{ "name": name, "value": "x" }]))));
  }
  let harmless = json!([{ "name": "RUST_LOG", "value": "info" }, { "name": "LC_ALL", "value": "C" }]);
  served.ok("set_project_environment", &set(harmless));
  served.let_automation_widen(true);
  served.ok("set_project_environment", &set(path.clone()));
  served.let_automation_widen(false);
  let mut more = path.as_array().expect("variables").clone();
  more.push(json!({ "name": "CI", "value": "1" }));
  served.ok("set_project_environment", &set(json!(more)));
  assert_person_only(&served.refused("set_project_environment", &set(json!([]))));
  served.let_automation_widen(true);
  served.ok("set_project_environment", &set(json!([])));
}

#[test]
fn domain_pipeline_steps_only_narrow_what_the_inspector_offers() {
  let mut served = served();
  let base = served.most_restrictive();
  served.let_automation_widen(true);
  served.ok(
    "save_profile_draft",
    &profile("Writer", Some(permissions(&base, "write", "any_in_worktree"))),
  );
  served.ok("publish_profile", &json!({ "project": "Kontur", "profile": "Writer" }));
  served.ok("publish_profile", &json!({ "project": "Kontur", "profile": "Probe" }));
  served.let_automation_widen(false);
  served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Narrow", "from": "empty" }),
  );
  let save = |step: Value| json!({ "project": "Kontur", "pipeline": "Narrow", "steps": [step] });
  let refused = served.refused(
    "save_pipeline_draft",
    &save(json!({ "key": "fix", "name": "Fix", "profile": "Writer", "narrow": { "only_commands": ["cargo test"] } })),
  );
  assert_eq!(refused["code"], "invalid_step", "{refused}");
  let refused = served.refused(
    "save_pipeline_draft",
    &save(json!({ "key": "fix", "name": "Fix", "profile": "Probe", "repository": "write" })),
  );
  assert_eq!(
    refused["code"], "invalid_step",
    "a read-only profile's step cannot write"
  );
  let refused = served.refused(
    "save_pipeline_draft",
    &save(json!({ "key": "fix", "name": "Fix", "profile": "Nobody" })),
  );
  assert_eq!(refused["code"], "invalid_step", "only published profiles");
  let saved = served.ok(
    "save_pipeline_draft",
    &save(json!({ "key": "fix", "name": "Fix", "profile": "Writer", "repository": "write", "narrow": { "ask_before_tool_calls": true } })),
  );
  assert_eq!(saved["pipeline"]["steps"][0]["narrow"]["ask_before_tool_calls"], true);
  let cycle = served.refused(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Narrow", "steps": [
      { "key": "a", "name": "Alpha", "runs_after": ["b"] },
      { "key": "b", "name": "Beta", "runs_after": ["a"] },
    ] }),
  );
  let reasons = cycle["reasons"].to_string();
  assert!(
    reasons.contains("Alpha") && reasons.contains("Beta"),
    "the cycle names its steps: {reasons}"
  );
}

#[test]
fn domain_pipeline_tools_tell_the_feed() {
  let mut served = served();
  let project = served.services().changes.clone();
  let counted = |changes: &orchester_workspace::tasks::outside::OutsideChanges| {
    changes
      .subscribe()
      .borrow()
      .values()
      .map(|counts| counts.pipelines)
      .sum::<u64>()
  };
  let before = counted(&project);
  served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Told", "from": "starter" }),
  );
  assert_eq!(counted(&project), before + 1, "create_pipeline");
  served.ok(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Told", "steps": [{ "key": "fix", "name": "Fix", "profile": "Builder" }] }),
  );
  assert_eq!(counted(&project), before + 2, "save_pipeline_draft");
  served.ok("publish_pipeline", &json!({ "project": "Kontur", "pipeline": "Told" }));
  assert_eq!(counted(&project), before + 3, "publish_pipeline");
}
