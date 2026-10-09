//! The command tool editor's fields: a tool reads back into the same fields, and each value the
//! rules reject says why in its own field.

use orchester_desktop::tools::form::{CommandText, tool_name};

fn filled() -> CommandText {
  CommandText {
    description: "Runs the tests.".into(),
    program: "cargo".into(),
    arguments: "test\n\n--workspace\nname with space".into(),
    timeout: "90".into(),
    exit_codes: "0, 3 1".into(),
    output_limit: "64".into(),
    environment: "RUST_LOG=info\n\nEMPTY=".into(),
  }
}

#[test]
fn tool_form_fields_become_a_command_tool_and_read_back() {
  let tool = filled().tool(None).expect("valid fields");
  let text = CommandText::of(&tool).expect("a command tool");
  assert_eq!(text.program, "cargo");
  assert_eq!(text.arguments, "test\n--workspace\nname with space");
  assert_eq!(text.exit_codes, "0, 1, 3");
  assert_eq!(text.environment, "EMPTY=\nRUST_LOG=info");
  assert_eq!(text.tool(None), Ok(tool));
  assert_eq!(CommandText::new_tool().timeout, "600");
}

#[test]
fn tool_form_says_which_field_is_wrong() {
  let wrong = CommandText {
    program: String::new(),
    timeout: "soon".into(),
    exit_codes: "zero".into(),
    output_limit: "0".into(),
    environment: "1ST=a".into(),
    ..filled()
  };
  let errors = wrong.tool(None).expect_err("invalid fields");
  assert_eq!(
    errors.program.map(|message| message.key),
    Some("tools.errors.programRequired")
  );
  assert_eq!(errors.timeout.map(|message| message.key), Some("tools.errors.timeout"));
  assert_eq!(
    errors.exit_codes.map(|message| message.key),
    Some("tools.errors.exitCodeNotNumber")
  );
  assert_eq!(
    errors.output_limit.map(|message| message.key),
    Some("tools.errors.outputLimit")
  );
  assert_eq!(
    errors.environment.map(|message| message.key),
    Some("tools.errors.variableName")
  );
  assert_eq!(errors.arguments, None);
  let duplicate = CommandText {
    environment: "A=1\nA=2".into(),
    ..filled()
  };
  assert_eq!(
    duplicate
      .tool(None)
      .expect_err("duplicate")
      .environment
      .map(|message| message.key),
    Some("tools.errors.duplicateVariable")
  );
  assert_eq!(
    tool_name("  ").map_err(|message| message.key),
    Err("errors.nameRequired")
  );
}
