//! The native tool catalog: every schema is strict-compatible and agrees with its typed input, and calls
//! are read strictly.

use orchester_core::{
  native::tools::{ToolInput, ToolInputError, ToolName, ToolSet, parse_call},
  permission::RepositoryAccess,
  run::StepOutcome,
};
use serde_json::{Value, json};

fn schema(tool: ToolName) -> Value {
  serde_json::from_str(tool.input_schema()).expect("schema is JSON")
}

/// An input with every property of `tool`'s schema: `null` where the schema allows it, else a value of
/// the property's type.
fn full_input(tool: ToolName) -> Value {
  let schema = schema(tool);
  let properties = schema["properties"].as_object().expect("properties");
  let input = properties.iter().map(|(name, property)| {
    let nullable = property["type"]
      .as_array()
      .is_some_and(|types| types.contains(&json!("null")));
    let value = match (nullable, name.as_str(), property["type"].as_str()) {
      (true, ..) => Value::Null,
      (false, "outcome", _) => json!("done"),
      (false, "edits", _) => json!([{"old": "a", "new": "b", "replace_all": null}]),
      (false, _, Some("string")) => json!("src/main.rs"),
      (false, _, other) => panic!("no sample for {name} of type {other:?}"),
    };
    (name.clone(), value)
  });
  Value::Object(input.collect())
}

#[test]
fn native_tools_schemas_are_strict() {
  for tool in ToolName::ALL {
    let schema = schema(tool);
    assert_eq!(schema["additionalProperties"], json!(false), "{tool:?}");
    let mut properties: Vec<&String> = schema["properties"].as_object().expect("properties").keys().collect();
    let mut required: Vec<&str> = schema["required"]
      .as_array()
      .expect("required")
      .iter()
      .map(|name| name.as_str().expect("name"))
      .collect();
    properties.sort();
    required.sort_unstable();
    assert_eq!(properties, required, "{tool:?}: every property is required");
    assert!(!tool.description().is_empty());
    assert_eq!(ToolName::from_name(tool.as_str()), Some(tool));
  }
}

#[test]
fn native_tools_schema_and_struct_agree() {
  for tool in ToolName::ALL {
    let input = full_input(tool);
    let parsed = parse_call(tool.as_str(), &input).unwrap_or_else(|error| panic!("{tool:?}: {error}"));
    assert_eq!(parsed.name(), tool);

    let mut extra = input.clone();
    extra["unexpected"] = json!(1);
    if tool != ToolName::FinishStep {
      assert!(
        parse_call(tool.as_str(), &extra).is_err(),
        "{tool:?} refuses unknown fields"
      );
    }
  }
}

#[test]
fn native_tools_defaults_and_missing_optionals() {
  let ToolInput::ReadFile(read) = parse_call("read_file", &json!({"path": "a.rs"})).expect("read") else {
    panic!("read_file");
  };
  assert_eq!((read.offset(), read.limit()), (1, 2000));
  let ToolInput::RunCommand(command) = parse_call("run_command", &json!({"command": "ls"})).expect("command") else {
    panic!("run_command");
  };
  assert_eq!(
    (command.shell(), command.workdir(), command.timeout_seconds()),
    (false, None, 120)
  );
  let ToolInput::FinishStep(result) =
    parse_call("finish_step", &json!({"outcome": "question", "summary": "Which one?"})).expect("finish")
  else {
    panic!("finish_step");
  };
  assert_eq!(result.outcome, StepOutcome::Question);
}

#[test]
fn native_tools_invalid_calls_name_the_problem() {
  let invalid = |name: &str, input: Value| match parse_call(name, &input) {
    Err(ToolInputError::Invalid { message, .. }) => message,
    other => panic!("{name}: {other:?}"),
  };
  assert!(invalid("read_file", json!({"path": " "})).contains("path"));
  assert!(invalid("read_file", json!({"path": "a", "offset": 0})).contains("0"));
  assert!(invalid("edit_file", json!({"path": "a", "edits": []})).contains("edits"));
  assert!(invalid("edit_file", json!({"path": "a", "edits": [{"old": "x", "new": "x"}]})).contains("changes nothing"));
  assert!(invalid("edit_file", json!({"path": "a", "edits": [{"old": "", "new": "x"}]})).contains("empty"));
  assert!(invalid("run_command", json!({"command": "ls", "timeout_seconds": 1801})).contains("1800"));
  assert!(invalid("finish_step", json!({"outcome": "done"})).contains("summary"));
  assert!(invalid("ask_question", json!({"question": ""})).contains("question"));
  assert_eq!(
    parse_call("delete_everything", &json!({})),
    Err(ToolInputError::UnknownTool("delete_everything".to_owned()))
  );
}

#[test]
fn native_tools_read_only_set_withholds_writes() {
  let read_only = ToolSet::for_access(RepositoryAccess::ReadOnly);
  assert!(!read_only.offers(ToolName::EditFile));
  assert!(!read_only.offers(ToolName::WriteFile));
  assert!(read_only.offers(ToolName::RunCommand));
  assert_eq!(ToolSet::for_access(RepositoryAccess::Write).tools(), ToolName::ALL);
}
