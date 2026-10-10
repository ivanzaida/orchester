//! The project's command tools through the domain tools: made, saved and published as their editor does,
//! the program found on the search path and stored as found. A command tool runs a program on this
//! computer whenever a pipeline step calls it, so making one, changing what it runs, publishing its first
//! version and adding a step that calls it are a person's decisions while *Let automation change agent
//! permissions* is off; its description, timeout and output limit are not.

use std::path::Path;

use serde_json::{Value, json};

use super::Served;

/// Kontur seeded empty, its Codex connection's program any file the seed records as checked.
fn served() -> Served {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  Served::seeded_with("empty", Some(codex)).0
}

/// The Check tool: `git grep --quiet hello`, found on the search path.
fn check(changes: Value) -> Value {
  let mut args = json!({
    "project": "Kontur",
    "name": "Check",
    "description": "Fails unless the repository says hello.",
    "program": "git",
    "arguments": ["grep", "--quiet", "hello"],
  });
  if let (Value::Object(args), Value::Object(changes)) = (&mut args, changes) {
    args.extend(changes);
  }
  args
}

/// [`check`] for the Check tool made already: named by `tool`, without its name.
fn change(changes: Value) -> Value {
  let mut args = check(changes);
  args["tool"] = json!("Check");
  args.as_object_mut().expect("arguments").remove("name");
  args
}

fn assert_person_only(error: &Value) {
  assert_eq!(error["code"], "person_only", "{error}");
  assert!(
    error["reasons"].as_array().is_some_and(|reasons| reasons.len() > 1),
    "{error}"
  );
}

#[test]
fn domain_a_command_tool_is_made_and_published_once_a_person_lets_it() {
  let mut served = served();
  assert_person_only(&served.refused("save_tool_draft", &check(json!({}))));
  served.let_automation_widen(true);
  let made = served.ok("save_tool_draft", &check(json!({})));
  let program = made["tool"]["command"]["program"].as_str().expect("a program");
  assert!(
    Path::new(program).is_absolute() && Path::new(program).is_file(),
    "stored as the file found: {program}"
  );
  assert_eq!(made["tool"]["state"]["state"], "draft");
  assert_eq!(made["tool"]["command"]["timeout_secs"], 600);
  assert_eq!(made["tool"]["command"]["exit_codes"], json!([0]));
  served.let_automation_widen(false);
  let publish = json!({ "project": "Kontur", "tool": "check" });
  assert_person_only(&served.refused("publish_tool", &publish));
  served.let_automation_widen(true);
  let published = served.ok("publish_tool", &publish);
  assert_eq!(published["tool"]["state"]["published_version"], 1, "{published}");
  served.let_automation_widen(false);
  let harmless = change(json!({
    "description": "Says hello.",
    "timeout_secs": 3600,
    "output_limit_kib": 512,
    "environment": [{ "name": "RUST_LOG", "value": "info" }],
  }));
  served.ok("save_tool_draft", &harmless);
  let second = served.ok("publish_tool", &publish);
  assert_eq!(second["tool"]["state"]["published_version"], 2, "{second}");
  assert_eq!(second["tool"]["command"]["timeout_secs"], 3600);
  for widening in [
    json!({ "arguments": ["grep", "--quiet", "bye"] }),
    json!({ "program": env!("CARGO") }),
    json!({ "exit_codes": [0, 1] }),
    json!({ "environment": [{ "name": "OPENAI_BASE_URL", "value": "http://localhost:1" }] }),
  ] {
    assert_person_only(&served.refused("save_tool_draft", &change(widening)));
  }
  let tools = served.ok("list_tools", &json!({ "project": "Kontur" }));
  let names: Vec<&str> = tools["tools"]
    .as_array()
    .expect("tools")
    .iter()
    .filter_map(|tool| tool["name"].as_str())
    .collect();
  assert!(names.contains(&"Check") && names.contains(&"Run checks"), "{names:?}");
}

#[test]
fn domain_a_tools_program_is_found_on_the_search_path_or_at_its_path() {
  let mut served = served();
  served.let_automation_widen(true);
  let in_repository = served.refused("save_tool_draft", &check(json!({ "program": "tools/check.ps1" })));
  assert_eq!(in_repository["code"], "program_in_worktree");
  let missing = served.refused(
    "save_tool_draft",
    &check(json!({ "program": "orchester-no-such-program" })),
  );
  assert_eq!(missing["code"], "program_not_found");
  let nowhere = served.folder.path().join("nowhere").join("check");
  let absent = served.refused(
    "save_tool_draft",
    &check(json!({ "program": nowhere.display().to_string() })),
  );
  assert_eq!(absent["code"], "program_not_found");
  for invalid in [
    json!({ "timeout_secs": 0 }),
    json!({ "exit_codes": [] }),
    json!({ "output_limit_kib": 0 }),
    json!({ "arguments": [""] }),
    json!({ "arguments": ["two\nlines"] }),
    json!({ "environment": [{ "name": "1BAD", "value": "x" }] }),
  ] {
    let refused = served.refused("save_tool_draft", &check(invalid.clone()));
    assert!(
      ["invalid_tool", "invalid_variable"].contains(&refused["code"].as_str().unwrap_or_default()),
      "{invalid}: {refused}"
    );
  }
  let absolute = served.ok("save_tool_draft", &check(json!({ "program": env!("CARGO") })));
  assert_eq!(absolute["tool"]["command"]["program"], env!("CARGO"));
  let taken = served.refused("save_tool_draft", &check(json!({ "name": "run checks" })));
  assert_eq!(taken["code"], "name_taken");
  let mut renamed = change(json!({}));
  renamed["name"] = json!("Other");
  assert_eq!(served.refused("save_tool_draft", &renamed)["code"], "name_not_editable");
  let unknown = served.refused("get_tool", &json!({ "project": "Kontur", "tool": "Nothing" }));
  assert_eq!(unknown["code"], "tool_not_found");
  let no_draft = served.refused("publish_tool", &json!({ "project": "Kontur", "tool": "Run checks" }));
  assert_eq!(no_draft["code"], "no_draft");
}

#[test]
fn domain_a_command_step_with_a_failure_route_needs_a_person() {
  let mut served = served();
  served.let_automation_widen(true);
  served.ok("save_tool_draft", &check(json!({})));
  served.ok("publish_tool", &json!({ "project": "Kontur", "tool": "Check" }));
  served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Change", "from": "empty" }),
  );
  served.let_automation_widen(false);
  let steps = |check_name: &str| {
    json!({ "project": "Kontur", "pipeline": "Change", "steps": [
      { "key": "implement", "name": "Implement", "profile": "Builder" },
      {
        "key": "check",
        "name": check_name,
        "tool": "Check",
        "runs_after": ["implement"],
        "on_failure": { "return_to": "implement", "retries": 2 },
      },
    ] })
  };
  let refused = served.refused("save_pipeline_draft", &steps("Check"));
  assert_person_only(&refused);
  assert!(
    refused["message"].as_str().is_some_and(|text| text.contains("Check")),
    "{refused}"
  );
  served.let_automation_widen(true);
  let saved = served.ok("save_pipeline_draft", &steps("Check"));
  let step = &saved["pipeline"]["steps"][1];
  assert_eq!(step["tool"]["name"], "Check", "{saved}");
  assert_eq!(step["on_failure"], json!({ "return_to": "implement", "retries": 2 }));
  served.let_automation_widen(false);
  served.ok("save_pipeline_draft", &steps("Run the checks"));
  let publish = json!({ "project": "Kontur", "pipeline": "Change" });
  assert_person_only(&served.refused("publish_pipeline", &publish));
  served.let_automation_widen(true);
  served.ok("publish_pipeline", &publish);
  let tool = served.ok("get_tool", &json!({ "project": "Kontur", "tool": "Check" }));
  assert_eq!(
    tool["tool"]["used_by_steps"],
    json!([{ "pipeline": "Change", "version": 1, "step": "Run the checks" }])
  );
}
