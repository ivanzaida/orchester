//! The calls both fake CLIs make for "Use the skill tools": `list_skills`; `read_skill` of the pinned skill
//! `run-checks` and its reference file, and of the repository's `deploy`; two `read_skill` calls whose file leaves
//! the skill (a `..` path and an absolute one); and `read_file` of the placed `run-checks/SKILL.md` by its absolute
//! path and of the repository's `deploy/SKILL.md`, which reach both kinds of skill too.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The `SKILL.md` of `run-checks` placed for the step execution, in `<repo>/.orchester/skills/<execution>/`.
fn placed(worktree: &Path) -> Option<PathBuf> {
  let skills = worktree.ancestors().nth(3)?.join(".orchester").join("skills");
  let execution = std::fs::read_dir(skills).ok()?.flatten().next()?.path();
  Some(execution.join("run-checks").join("SKILL.md"))
}

/// Each call: its id, its tool and its input.
pub fn calls(worktree: &Path) -> Vec<(&'static str, &'static str, Value)> {
  let placed = placed(worktree).map(|path| path.to_string_lossy().into_owned());
  vec![
    ("toolu_skills", "list_skills", json!({})),
    ("toolu_skill", "read_skill", json!({"name": "run-checks"})),
    (
      "toolu_skill_file",
      "read_skill",
      json!({"name": "run-checks", "file": "references/checks.md"}),
    ),
    (
      "toolu_repository_skill",
      "read_skill",
      json!({"name": "deploy", "file": "references/steps.md"}),
    ),
    (
      "toolu_skill_up",
      "read_skill",
      json!({"name": "run-checks", "file": "../../../outside-notes.txt"}),
    ),
    (
      "toolu_skill_absolute",
      "read_skill",
      json!({"name": "deploy", "file": placed}),
    ),
    (
      "toolu_read_placed",
      "read_file",
      json!({"filename": placed, "start_line": null, "length": null}),
    ),
    (
      "toolu_read_repository",
      "read_file",
      json!({"filename": ".agents/skills/deploy/SKILL.md"}),
    ),
  ]
}
