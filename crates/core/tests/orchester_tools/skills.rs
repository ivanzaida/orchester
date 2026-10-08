//! `list_skills` and `read_skill`: strict input with skill file paths, and read-only tools offered only with skills.

use orchester_core::{
  orchester_tools::{AccessClass, OrchesterTool, SkillCall, StepTools, ToolCall, ToolCallError, ToolPolicy, decide},
  resource::{RepositoryAccess, SkillFilePath},
};
use serde_json::{Value, json};

use crate::{permissions, reading, writing};

fn read_skill(arguments: &Value) -> Result<ToolCall, ToolCallError> {
  ToolCall::parse("read_skill", arguments, &writing().with_skills())
}

fn invalid(arguments: &Value) -> String {
  match read_skill(arguments) {
    Err(ToolCallError::Invalid { tool, message }) => {
      assert_eq!(tool, OrchesterTool::ReadSkill);
      message
    }
    other => panic!("{arguments} is refused as invalid, not {other:?}"),
  }
}

#[test]
fn orchester_tools_read_skill_reads_a_name_and_a_skill_file_path() {
  assert_eq!(
    read_skill(&json!({"name": "run-checks"})).expect("valid"),
    ToolCall::Skill(SkillCall::Read {
      name: "run-checks".to_owned(),
      file: None,
      offset: 1,
      limit: 2000
    })
  );
  assert_eq!(
    read_skill(&json!({"name": "run-checks", "file": "references/checks.md", "offset": 3, "limit": 5})).expect("valid"),
    ToolCall::Skill(SkillCall::Read {
      name: "run-checks".to_owned(),
      file: Some(SkillFilePath::new("references/checks.md").expect("a path")),
      offset: 3,
      limit: 5
    })
  );
  // The skill's own document is its SKILL.md, named or not.
  assert!(matches!(
    read_skill(&json!({"name": "run-checks", "file": "SKILL.md"})),
    Ok(ToolCall::Skill(SkillCall::Read { file: None, .. }))
  ));
  assert_eq!(
    ToolCall::parse("list_skills", &Value::Null, &reading().with_skills())
      .expect("valid")
      .tool(),
    OrchesterTool::ListSkills
  );
}

#[test]
fn orchester_tools_read_skill_refuses_paths_that_leave_the_skill() {
  for file in [
    "../other/SKILL.md",
    "references/../../x.md",
    "/etc/passwd",
    "C:/Windows/win.ini",
    "C:\\Windows\\win.ini",
    "references\\checks.md",
    "",
    "./checks.md",
  ] {
    let message = invalid(&json!({"name": "run-checks", "file": file}));
    assert!(message.contains("reference file path"), "{file}: {message}");
  }
  assert!(invalid(&json!({"name": " "})).contains("not a skill name"));
  assert!(invalid(&json!({"name": "a\nb"})).contains("not a skill name"));
  assert!(invalid(&json!({"name": "x", "path": "/etc/passwd"})).contains("unknown field"));
  assert!(invalid(&json!({"name": "x", "limit": 10_001})).contains("at most"));
  assert!(matches!(
    ToolCall::parse("list_skills", &json!({"name": "x"}), &writing().with_skills()),
    Err(ToolCallError::Invalid { .. })
  ));
}

/// Both tools read and never ask, in every step, but are offered only to a step that has skills.
#[test]
fn orchester_tools_skill_tools_are_reads_offered_with_skills() {
  for tool in [OrchesterTool::ListSkills, OrchesterTool::ReadSkill] {
    assert_eq!(tool.access(), AccessClass::ReadOnly, "{tool:?}");
    assert!(tool.read_only(), "{tool:?}");
    for access in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
      assert_eq!(decide(tool, &permissions(access)), Some(ToolPolicy::Runs), "{tool:?}");
    }
    assert_eq!(reading().policy(tool), None, "{tool:?}");
    assert_eq!(reading().with_skills().policy(tool), Some(ToolPolicy::Runs), "{tool:?}");
  }
  assert!(matches!(
    ToolCall::parse("read_skill", &json!({"name": "x"}), &writing()),
    Err(ToolCallError::NotOffered(OrchesterTool::ReadSkill))
  ));
  let tools = reading().with_skills();
  let rules = tools.claude_allow_rules();
  assert!(rules.contains(&"mcp__Orchester__read_skill".to_owned()), "{rules:?}");
  assert!(rules.contains(&"mcp__Orchester__list_skills".to_owned()), "{rules:?}");
  assert!(tools.codex_approved().contains(&"read_skill"));
  let listed = tools.mcp_tools();
  let read = listed
    .iter()
    .find(|tool| tool["name"] == "read_skill")
    .expect("read_skill listed");
  assert_eq!(read["annotations"]["readOnlyHint"], true);
  assert_eq!(read["inputSchema"]["additionalProperties"], false);
  assert_eq!(read["inputSchema"]["required"], json!(["name"]));
}

#[test]
fn orchester_tools_prompt_paragraph_points_at_read_skill_when_offered() {
  let codex = reading().with_skills().prompt_paragraph(false);
  assert!(
    codex.contains("Read your skills with read_skill by name, never by path; list_skills lists them."),
    "{codex}"
  );
  let claude = StepTools::for_step(&permissions(RepositoryAccess::Write), &[])
    .with_skills()
    .prompt_paragraph(true);
  assert!(
    claude.contains("Read your skills with mcp__Orchester__read_skill by name"),
    "{claude}"
  );
  assert!(!reading().prompt_paragraph(false).contains("skills"));
}
