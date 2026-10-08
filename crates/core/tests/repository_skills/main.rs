//! The repository's Codex skills found for Codex to disable while the project has its agent files off, and loaded
//! for a native agent while they are on (`orchester_core::agent_files::skills`): every `SKILL.md` of
//! `.agents/skills` and `.codex/skills` through links within the worktree, by each path that reaches it and resolved,
//! and a refusal, by where it lies in the worktree, for a link out, an unreadable folder or a search past its bounds.

mod native_skills;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::agent_files::skills::{RepositorySkills, RepositorySkillsError};

pub fn write(path: &Path, text: &str) {
  fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
  fs::write(path, text).expect("a file");
}

/// A link to the folder `target` at `link`: a junction on Windows, which needs no privilege, a symbolic link on Unix.
pub fn link_folder(target: &Path, link: &Path) {
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

/// `path` with its links resolved, as the search names it: without Windows' `\\?\` prefix.
pub fn resolved(path: &Path) -> PathBuf {
  let resolved = fs::canonicalize(path).expect("the path exists");
  let text = resolved.to_string_lossy();
  PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

/// A worktree with `.agents/skills/deploy` (and a nested skill of its own) and `.codex/skills/review`, beside a
/// folder outside it that holds a skill.
pub struct Repository {
  _root: tempfile::TempDir,
  pub worktree: PathBuf,
  pub outside: PathBuf,
}

impl Default for Repository {
  fn default() -> Self {
    Self::new()
  }
}

impl Repository {
  pub fn new() -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let worktree = root.path().join("worktree");
    let outside = root.path().join("outside");
    for (path, name) in [
      (worktree.join(".agents/skills/deploy/SKILL.md"), "deploy"),
      (worktree.join(".agents/skills/deploy/nested/SKILL.md"), "nested"),
      (worktree.join(".codex/skills/review/SKILL.md"), "review"),
      (outside.join("escaped/SKILL.md"), "escaped"),
    ] {
      write(&path, &format!("---\nname: {name}\ndescription: Does {name}.\n---\n"));
    }
    write(&worktree.join(".agents/skills/deploy/references/steps.md"), "steps\n");
    Self {
      _root: root,
      worktree,
      outside,
    }
  }

  pub fn path(&self, relative: &str) -> PathBuf {
    relative
      .split('/')
      .fold(self.worktree.clone(), |path, name| path.join(name))
  }

  fn codex_paths(&self) -> Vec<PathBuf> {
    RepositorySkills::find(&self.worktree)
      .expect("the skills are found")
      .codex_paths()
  }

  fn refusal(&self) -> RepositorySkillsError {
    RepositorySkills::find(&self.worktree).expect_err("the search refuses")
  }

  /// Each of `relative` by its path through the worktree and resolved, sorted, each once.
  fn both_paths(&self, relative: &[&str]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = relative
      .iter()
      .flat_map(|relative| [self.path(relative), resolved(&self.path(relative))])
      .collect();
    paths.sort();
    paths.dedup();
    paths
  }
}

#[test]
fn repository_skills_lists_every_skill_by_both_paths_for_codex_to_disable() {
  let repository = Repository::new();
  write(&repository.path("tools/shared/SKILL.md"), "---\nname: shared\n---\n");
  link_folder(
    &repository.path("tools/shared"),
    &repository.path(".agents/skills/shared"),
  );
  let paths = repository.codex_paths();
  assert_eq!(
    paths,
    repository.both_paths(&[
      ".agents/skills/deploy/SKILL.md",
      ".agents/skills/deploy/nested/SKILL.md",
      ".agents/skills/shared/SKILL.md",
      ".codex/skills/review/SKILL.md",
    ])
  );
  // Codex names a skill reached through a link by its resolved path, and the search names it by both.
  assert!(
    paths.contains(&repository.path(".agents/skills/shared/SKILL.md")),
    "{paths:?}"
  );
  assert!(
    paths.contains(&resolved(&repository.path("tools/shared/SKILL.md"))),
    "{paths:?}"
  );
}

#[test]
fn repository_skills_lists_a_folder_reached_by_two_paths_by_both() {
  let repository = Repository::new();
  fs::remove_dir_all(repository.path(".codex/skills")).expect("removed");
  link_folder(&repository.path(".agents/skills"), &repository.path(".codex/skills"));
  assert_eq!(
    repository.codex_paths(),
    repository.both_paths(&[
      ".agents/skills/deploy/SKILL.md",
      ".agents/skills/deploy/nested/SKILL.md",
      ".codex/skills/deploy/SKILL.md",
      ".codex/skills/deploy/nested/SKILL.md",
    ])
  );
}

#[test]
fn repository_skills_finds_none_without_skills_folders() {
  let root = tempfile::tempdir().expect("a temporary folder");
  let skills = RepositorySkills::find(root.path()).expect("nothing to find");
  assert_eq!(skills.codex_paths(), Vec::<PathBuf>::new());
}

#[test]
fn repository_skills_walks_hidden_folders_but_never_a_git_entry() {
  let repository = Repository::new();
  write(
    &repository.path(".agents/skills/.hidden/SKILL.md"),
    "---\nname: hidden\n---\n",
  );
  write(
    &repository.path(".agents/skills/vendor/.git/SKILL.md"),
    "---\nname: git\n---\n",
  );
  // A nested repository's `.git` holds more folders than a search reads; it does not use up the bound.
  for index in 0..600 {
    fs::create_dir_all(repository.path(&format!(".agents/skills/vendor/.git/objects/{index:03}"))).expect("a folder");
  }
  let paths = repository.codex_paths();
  assert!(
    paths.contains(&repository.path(".agents/skills/.hidden/SKILL.md")),
    "{paths:?}"
  );
  assert!(
    !paths.iter().any(|path| path.to_string_lossy().contains(".git")),
    "{paths:?}"
  );
}

#[test]
fn repository_skills_refuses_a_link_loop_at_the_depth_bound() {
  let repository = Repository::new();
  link_folder(
    &repository.path(".agents/skills"),
    &repository.path(".agents/skills/deploy/loop"),
  );
  // Every path to a folder is followed, so the loop runs until the depth bound refuses it.
  assert_eq!(
    repository.refusal(),
    RepositorySkillsError::TooDeep(".agents/skills/deploy/loop/deploy/loop/deploy/loop/deploy/loop/deploy".to_owned())
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
fn repository_skills_refuses_a_skills_folder_it_cannot_read() {
  let repository = Repository::new();
  // A file where the skills folder should be cannot be read as a folder, on every platform.
  fs::remove_dir_all(repository.path(".codex/skills")).expect("removed");
  fs::write(repository.path(".codex/skills"), "not a folder").expect("a file");
  let refusal = repository.refusal();
  assert!(
    matches!(&refusal, RepositorySkillsError::Unreadable { path, .. } if path == ".codex/skills"),
    "{refusal:?}"
  );
  let message = refusal.refusal_while_off();
  assert!(
    message.starts_with("Repository agent files are off, but `.codex/skills` cannot be read ("),
    "{message}"
  );
  assert!(
    message.ends_with(
      "), so Orchester cannot disable the skills Codex could load from it. Make it readable or turn the project's \
       Repository agent files on."
    ),
    "{message}"
  );
}

#[cfg(unix)]
#[test]
fn repository_skills_refuses_a_folder_without_permission() {
  use std::os::unix::fs::PermissionsExt;

  let repository = Repository::new();
  let folder = repository.path(".agents/skills/deploy");
  fs::set_permissions(&folder, fs::Permissions::from_mode(0o000)).expect("permissions");
  // A superuser reads it all the same: nothing to check then.
  let readable = fs::read_dir(&folder).is_ok();
  let refusal = RepositorySkills::find(&repository.worktree).map(|skills| skills.codex_paths());
  fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).expect("permissions");
  if !readable {
    assert!(
      matches!(&refusal, Err(RepositorySkillsError::Unreadable { path, .. }) if path == ".agents/skills/deploy"),
      "{refusal:?}"
    );
  }
}

#[test]
fn repository_skills_refuses_past_the_depth_bound() {
  let repository = Repository::new();
  // Eight folders below the skills folder are searched.
  write(
    &repository.path(".agents/skills/a/b/c/d/e/f/g/h/SKILL.md"),
    "---\nname: deep\n---\n",
  );
  let paths = repository.codex_paths();
  assert!(
    paths.contains(&repository.path(".agents/skills/a/b/c/d/e/f/g/h/SKILL.md")),
    "{paths:?}"
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
fn repository_skills_refuses_more_than_500_folders() {
  let root = tempfile::tempdir().expect("a temporary folder");
  let skills = root.path().join(".agents").join("skills");
  // 500 folders below the skills folders are read; the skills folders themselves do not count.
  for index in 0..500 {
    fs::create_dir_all(skills.join(format!("skill-{index:03}"))).expect("a folder");
  }
  fs::create_dir_all(root.path().join(".codex").join("skills")).expect("a folder");
  assert!(RepositorySkills::find(root.path()).is_ok());

  fs::create_dir_all(skills.join("skill-500")).expect("one folder more");
  let refusal = RepositorySkills::find(root.path()).expect_err("the search refuses");
  assert_eq!(refusal, RepositorySkillsError::TooManyFolders);
  assert_eq!(
    refusal.refusal_while_off(),
    "Repository agent files are off, but the skills folders lead to more than 500 folders, each path to a folder \
     counted, so Codex could load skills there that Orchester does not look for. Make the skills folders smaller or \
     turn the project's Repository agent files on."
  );
}
