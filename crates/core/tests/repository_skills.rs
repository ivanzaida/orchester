//! The repository's Codex skills found for Codex to disable while the project has its agent files off
//! (`orchester_core::agent_files::skills`): every `SKILL.md` of `.agents/skills` and `.codex/skills` through links
//! within the worktree, and a refusal, by where it lies in the worktree, for a link out or a search past its bounds.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::agent_files::skills::{RepositorySkillsError, repository_skills};

fn write(path: &Path, text: &str) {
  fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
  fs::write(path, text).expect("a file");
}

/// A link to the folder `target` at `link`: a junction on Windows, which needs no privilege, a symbolic link on Unix.
fn link_folder(target: &Path, link: &Path) {
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link)
      .arg(target)
      .output()
      .expect("cmd runs");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("a link");
}

/// A worktree with `.agents/skills/deploy` (and a nested skill of its own) and `.codex/skills/review`, beside a
/// folder outside it that holds a skill.
struct Repository {
  _root: tempfile::TempDir,
  worktree: PathBuf,
  outside: PathBuf,
}

impl Repository {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let worktree = root.path().join("worktree");
    let outside = root.path().join("outside");
    for path in [
      worktree.join(".agents/skills/deploy/SKILL.md"),
      worktree.join(".agents/skills/deploy/nested/SKILL.md"),
      worktree.join(".codex/skills/review/SKILL.md"),
      outside.join("escaped/SKILL.md"),
    ] {
      write(&path, "---\nname: skill\n---\n");
    }
    write(&worktree.join(".agents/skills/deploy/references/steps.md"), "steps\n");
    Self {
      _root: root,
      worktree,
      outside,
    }
  }

  fn path(&self, relative: &str) -> PathBuf {
    relative
      .split('/')
      .fold(self.worktree.clone(), |path, name| path.join(name))
  }

  fn refusal(&self) -> RepositorySkillsError {
    repository_skills(&self.worktree).expect_err("the search refuses")
  }
}

#[test]
fn repository_skills_lists_every_skill_for_codex_to_disable() {
  let repository = Repository::new();
  write(&repository.path("tools/shared/SKILL.md"), "---\nname: shared\n---\n");
  // A link within the worktree is followed, and its skills are named through it, as Codex finds them.
  link_folder(
    &repository.path("tools/shared"),
    &repository.path(".agents/skills/shared"),
  );
  assert_eq!(
    repository_skills(&repository.worktree).expect("the skills are found"),
    [
      ".agents/skills/deploy/SKILL.md",
      ".agents/skills/deploy/nested/SKILL.md",
      ".agents/skills/shared/SKILL.md",
      ".codex/skills/review/SKILL.md",
    ]
    .map(|relative| repository.path(relative))
  );
}

#[test]
fn repository_skills_finds_none_without_skills_folders() {
  let root = tempfile::tempdir().expect("a temporary folder");
  assert_eq!(repository_skills(root.path()), Ok(Vec::new()));
}

#[test]
fn repository_skills_reads_a_link_loop_inside_the_worktree_once() {
  let repository = Repository::new();
  link_folder(
    &repository.path(".agents/skills"),
    &repository.path(".agents/skills/deploy/loop"),
  );
  link_folder(
    &repository.path(".agents/skills/deploy"),
    &repository.path(".agents/skills/deploy/nested/back"),
  );
  assert_eq!(
    repository_skills(&repository.worktree).expect("a loop does not refuse"),
    [
      ".agents/skills/deploy/SKILL.md",
      ".agents/skills/deploy/nested/SKILL.md",
      ".codex/skills/review/SKILL.md",
    ]
    .map(|relative| repository.path(relative))
  );
}

/// The refusal names the link where it lies in the worktree, never where it leads.
fn assert_refused_as_leading_out(repository: &Repository, link: &str) {
  let refusal = repository.refusal();
  assert_eq!(refusal, RepositorySkillsError::LeadsOut(link.to_owned()));
  let message = refusal.refusal_while_off();
  assert_eq!(
    message,
    format!(
      "Repository agent files are off, but `{link}` leads out of the worktree, so Codex could load skills from it. \
       Remove the link or turn the project's Repository agent files on."
    )
  );
  let outside = repository.outside.to_string_lossy().into_owned();
  assert!(!message.contains(&outside), "{message}");
  assert!(!message.contains("outside"), "{message}");
}

#[cfg(windows)]
#[test]
fn repository_skills_refuses_a_junction_out_of_the_worktree() {
  let repository = Repository::new();
  link_folder(&repository.outside, &repository.path(".agents/skills/x"));
  assert_refused_as_leading_out(&repository, ".agents/skills/x");

  // A skills folder that is itself a junction out refuses as well.
  let repository = Repository::new();
  fs::remove_dir_all(repository.path(".codex/skills")).expect("removed");
  link_folder(&repository.outside, &repository.path(".codex/skills"));
  assert_refused_as_leading_out(&repository, ".codex/skills");
}

#[cfg(unix)]
#[test]
fn repository_skills_refuses_a_link_out_of_the_worktree() {
  let repository = Repository::new();
  link_folder(&repository.outside, &repository.path(".agents/skills/deploy/more"));
  assert_refused_as_leading_out(&repository, ".agents/skills/deploy/more");

  // A `SKILL.md` that links to a file out of the worktree refuses as well.
  let repository = Repository::new();
  fs::create_dir_all(repository.path(".codex/skills/linked")).expect("a folder");
  std::os::unix::fs::symlink(
    repository.outside.join("escaped").join("SKILL.md"),
    repository.path(".codex/skills/linked/SKILL.md"),
  )
  .expect("a file link");
  assert_refused_as_leading_out(&repository, ".codex/skills/linked/SKILL.md");
}

#[test]
fn repository_skills_refuses_past_the_depth_bound() {
  let repository = Repository::new();
  // Eight folders below the skills folder are searched.
  write(
    &repository.path(".agents/skills/a/b/c/d/e/f/g/h/SKILL.md"),
    "---\nname: deep\n---\n",
  );
  let found = repository_skills(&repository.worktree).expect("eight folders deep is searched");
  assert!(
    found.contains(&repository.path(".agents/skills/a/b/c/d/e/f/g/h/SKILL.md")),
    "{found:?}"
  );

  fs::create_dir_all(repository.path(".agents/skills/a/b/c/d/e/f/g/h/i")).expect("a ninth folder");
  let refusal = repository.refusal();
  assert_eq!(
    refusal,
    RepositorySkillsError::TooDeep(".agents/skills/a/b/c/d/e/f/g/h/i".to_owned())
  );
  assert_eq!(
    refusal.refusal_while_off(),
    "Repository agent files are off, but `.agents/skills/a/b/c/d/e/f/g/h/i` lies more than 8 folders deep in its \
     skills folder, so Codex could load skills there that Orchester does not look for. Make the skills folders \
     smaller or turn the project's Repository agent files on."
  );
}

#[test]
fn repository_skills_refuses_past_the_folder_bound() {
  let root = tempfile::tempdir().expect("a temporary folder");
  let skills = root.path().join(".agents").join("skills");
  // The skills folder and 500 folders in it: one more than a search reads.
  for index in 0..500 {
    fs::create_dir_all(skills.join(format!("skill-{index:03}"))).expect("a folder");
  }
  let refusal = repository_skills(root.path()).expect_err("the search refuses");
  assert_eq!(refusal, RepositorySkillsError::TooManyFolders);
  assert_eq!(
    refusal.refusal_while_off(),
    "Repository agent files are off, but the skills folders hold more than 500 folders, so Codex could load skills \
     there that Orchester does not look for. Make the skills folders smaller or turn the project's Repository agent \
     files on."
  );
}
