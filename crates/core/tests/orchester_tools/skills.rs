//! `list_skills` and `read_skill`: strict input with skill file paths, read-only tools offered only with skills,
//! and reads that stay within each skill's own files.

use std::{fs, path::Path};

use orchester_core::{
  orchester_tools::{
    AccessClass, OrchesterTool, SkillCall, StepSkills, StepTools, ToolCall, ToolCallError, ToolPolicy, decide,
  },
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

fn write(path: &Path, text: &str) {
  fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
  fs::write(path, text).expect("a file");
}

/// A repository with a worktree whose `.claude/skills` holds `deploy` (a reference file, a nested skill of its own
/// and a `.git` entry) and `run-checks`, and a placed folder holding Orchester's `run-checks` with one listed
/// reference file and one file its version does not list.
struct Skills {
  _root: tempfile::TempDir,
  worktree: std::path::PathBuf,
  placed: std::path::PathBuf,
}

impl Skills {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let worktree = root.path().join("worktree");
    let placed = root.path().join(".orchester").join("skills").join("e1");
    let deploy = worktree.join(".claude").join("skills").join("deploy");
    write(
      &deploy.join("SKILL.md"),
      "---\nname: deploy\ndescription: Deploys\n  the app.\n---\nDeploy it.\n",
    );
    write(&deploy.join("references").join("steps.md"), "step one\nstep two\n");
    write(
      &deploy.join("nested").join("SKILL.md"),
      "---\nname: nested\ndescription: Nested.\n---\n",
    );
    write(&deploy.join("nested").join("inner.md"), "inner\n");
    write(&deploy.join(".git").join("config"), "[remote]\n");
    write(
      &worktree
        .join(".claude")
        .join("skills")
        .join("run-checks")
        .join("SKILL.md"),
      "---\nname: run-checks\ndescription: The repository's checks.\n---\n",
    );
    let checks = placed.join("run-checks");
    write(
      &checks.join("SKILL.md"),
      "---\nname: \"run-checks\"\ndescription: \"Runs the checks.\"\n---\n\nRun them.\n",
    );
    write(&checks.join("references").join("checks.md"), "cargo test\n");
    write(&checks.join("references").join("unlisted.md"), "not listed\n");
    write(
      &root.path().join("outside").join("SKILL.md"),
      "---\nname: outside\n---\n",
    );
    Self {
      worktree,
      placed,
      _root: root,
    }
  }

  fn offered(&self) -> StepSkills {
    let mut skills = StepSkills::default();
    let listed = [SkillFilePath::new("references/checks.md").expect("a path")];
    skills
      .add_placed(
        ("run-checks", "Runs the checks."),
        &self.placed.join("run-checks").join("SKILL.md"),
        &listed,
      )
      .expect("placed");
    let repository = self.worktree.join(".claude").join("skills");
    for document in [
      repository.join("deploy").join("SKILL.md"),
      repository.join("run-checks").join("SKILL.md"),
      self.worktree.join("..").join("outside").join("SKILL.md"),
    ] {
      skills.add_repository(&document, &self.worktree).expect("offered");
    }
    skills
  }
}

fn read(skills: &StepSkills, name: &str, file: Option<&str>) -> Result<String, String> {
  let call = SkillCall::Read {
    name: name.to_owned(),
    file: file.map(|file| SkillFilePath::new(file).expect("a path")),
    offset: 1,
    limit: 2000,
  };
  let result = skills.run(&call);
  if result.is_error() {
    Err(result.text().to_owned())
  } else {
    Ok(result.text().to_owned())
  }
}

#[test]
fn orchester_tools_list_skills_names_each_skill_with_its_source_and_files() {
  let skills = Skills::new();
  let listed = skills.offered().run(&SkillCall::List);
  assert!(!listed.is_error());
  let text = listed.text();
  assert!(
    text.contains("- run-checks (orchester): Runs the checks.\n  reference files: references/checks.md\n"),
    "{text}"
  );
  assert!(
    text.contains("- deploy (repository): Deploys the app.\n  reference files: references/steps.md\n"),
    "{text}"
  );
  assert!(!text.contains("unlisted.md") && !text.contains("inner.md"), "{text}");
  assert!(
    text.contains("run-checks/SKILL.md: the name `run-checks` is another skill's"),
    "{text}"
  );
  assert!(
    text.contains("outside/SKILL.md: its folder leads out of the worktree"),
    "{text}"
  );
  assert!(!text.contains(".orchester"), "{text}");
}

#[test]
fn orchester_tools_read_skill_reads_a_skills_own_files() {
  let skills = Skills::new();
  let offered = skills.offered();
  let document = read(&offered, "run-checks", None).expect("SKILL.md");
  assert!(document.contains("\tname: \"run-checks\""), "{document}");
  assert!(document.contains("\tRun them."), "{document}");
  assert_eq!(
    read(&offered, "run-checks", Some("references/checks.md")).expect("a listed file"),
    "     1\tcargo test\n"
  );
  assert_eq!(
    read(&offered, "deploy", Some("references/steps.md")).expect("a file of the folder"),
    "     1\tstep one\n     2\tstep two\n"
  );
  assert!(read(&offered, "deploy", None).expect("SKILL.md").contains("Deploy it."));
}

/// What a skill does not hold is refused: a file its version does not list, a file of a nested skill, a `.git`
/// entry, a missing file, a skill the step does not have.
#[test]
fn orchester_tools_read_skill_refuses_what_is_not_the_skills() {
  let skills = Skills::new();
  let offered = skills.offered();
  for (name, file) in [
    ("run-checks", "references/unlisted.md"),
    ("deploy", "nested/inner.md"),
    ("deploy", "nested/SKILL.md"),
    ("deploy", ".git/config"),
    ("deploy", "references/missing.md"),
  ] {
    let refused = read(&offered, name, Some(file)).expect_err(file);
    assert_eq!(
      refused,
      format!("`{file}` is not a reference file of the skill `{name}`: list_skills lists its files")
    );
  }
  assert_eq!(
    read(&offered, "outside", None).expect_err("left out"),
    "this step has no skill named `outside`: list_skills names its skills"
  );
  assert!(StepSkills::default().is_empty());
  assert!(!offered.is_empty());
}
