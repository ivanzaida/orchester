//! The repository's skills a native agent's prompt lists while the project has its agent files on: each once, read
//! only as a regular file within its cap, and a refusal for a link into the worktree's `.git` or `.orchester` and for
//! what refuses the search.

use std::fs;

use orchester_core::agent_files::skills::{
  RepositorySkillsError,
  native::{NativeSkill, NativeSkillsError, native_skills},
};

use crate::{Repository, link_folder, write};

fn skill(name: &str, description: &str, path: &str) -> NativeSkill {
  NativeSkill {
    name: name.to_owned(),
    description: description.to_owned(),
    path: path.to_owned(),
  }
}

#[test]
fn native_skills_lists_each_skill_once_by_its_first_path() {
  let repository = Repository::new();
  write(&repository.path(".agents/skills/plain/SKILL.md"), "No front matter.\n");
  fs::remove_dir_all(repository.path(".codex/skills")).expect("removed");
  link_folder(&repository.path(".agents/skills"), &repository.path(".codex/skills"));
  assert_eq!(
    native_skills(&repository.worktree).expect("the skills load"),
    [
      skill("deploy", "Does deploy.", ".agents/skills/deploy/SKILL.md"),
      skill("nested", "Does nested.", ".agents/skills/deploy/nested/SKILL.md"),
      // Without a name in its front matter a skill is called by its folder.
      skill("plain", "", ".agents/skills/plain/SKILL.md"),
    ]
  );
}

#[test]
fn native_skills_read_at_most_64_kib_of_a_skill() {
  let repository = Repository::new();
  let long = "x".repeat(70 * 1024);
  write(
    &repository.path(".agents/skills/capped/SKILL.md"),
    &format!("---\nname: big\ndescription: {long}\n---\n"),
  );
  let skills = native_skills(&repository.worktree).expect("the skills load");
  // The front matter's end lies past what is read, so the skill is called by its folder.
  assert!(
    skills.contains(&skill("capped", "", ".agents/skills/capped/SKILL.md")),
    "{skills:?}"
  );
}

#[test]
fn native_skills_refuse_a_link_into_git_or_orchester() {
  for state in [".git", ".orchester"] {
    let repository = Repository::new();
    write(
      &repository.path(&format!("{state}/kept/SKILL.md")),
      "---\nname: kept\n---\n",
    );
    link_folder(
      &repository.path(&format!("{state}/kept")),
      &repository.path(".agents/skills/x"),
    );
    let refusal = native_skills(&repository.worktree).expect_err("the load refuses");
    assert_eq!(
      refusal,
      NativeSkillsError::Protected(".agents/skills/x/SKILL.md".to_owned())
    );
    assert_eq!(
      refusal.to_string(),
      "cannot load the repository's skills: `.agents/skills/x/SKILL.md` leads into the worktree's .git or .orchester"
    );
  }
}

#[test]
fn native_skills_refuse_what_refuses_the_search() {
  let repository = Repository::new();
  fs::create_dir_all(repository.path(".agents/skills/a/b/c/d/e/f/g/h/i")).expect("nine folders deep");
  let refusal = native_skills(&repository.worktree).expect_err("the load refuses");
  assert_eq!(
    refusal,
    NativeSkillsError::Search(RepositorySkillsError::TooDeep(
      ".agents/skills/a/b/c/d/e/f/g/h/i".to_owned()
    ))
  );
  assert_eq!(
    refusal.to_string(),
    "cannot load the repository's skills: `.agents/skills/a/b/c/d/e/f/g/h/i` lies more than 8 folders deep in its \
     skills folder"
  );
}

#[cfg(unix)]
#[test]
fn native_skills_never_open_a_pipe() {
  let repository = Repository::new();
  let folder = repository.path(".agents/skills/fifo");
  fs::create_dir_all(&folder).expect("a folder");
  let made = std::process::Command::new("mkfifo")
    .arg(folder.join("SKILL.md"))
    .status()
    .expect("mkfifo runs");
  assert!(made.success());
  assert_eq!(
    native_skills(&repository.worktree),
    Err(NativeSkillsError::NotRegular(".agents/skills/fifo/SKILL.md".to_owned()))
  );
}
