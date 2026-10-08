//! The repository's skills a step offers, found in the skills folders the runner names as the agent's CLI finds
//! them (every `SKILL.md` at any depth, through links), but only within the worktree and within bounds: a folder or
//! document that leads out of the worktree, into its `.git` or `.orchester`, cannot be read or lies past the bounds
//! is left out, and nothing found there stops the agent's start. A `SKILL.md` is read only once it resolves to a
//! regular file of its folder in the worktree, by its resolved path, and a skill whose name is not one line of
//! plain text is left out without its name.

use std::{
  collections::HashSet,
  ffi::OsString,
  fs::{self, File},
  io::{self, Read},
  path::{Path, PathBuf},
};

use super::{OfferedSkill, SkillSource, StepSkills, belongs, is_regular_file};
use crate::{
  agent_files::{
    frontmatter,
    skills::{MAX_HEADER_BYTES, MAX_SEARCH_DEPTH, MAX_SEARCHED_FOLDERS, shown_in_worktree},
  },
  native::scope::protected,
  permission::canonical,
  resource::{SKILL_FILE, SkillFilePath},
};

/// The most repository skills one step offers.
const MAX_REPOSITORY_SKILLS: usize = 100;
/// The longest repository skill name offered, in characters.
const MAX_NAME_CHARS: usize = 200;
/// The most reference files `list_skills` names for one repository skill.
const MAX_LISTED_FILES: usize = 200;
/// How deep below its folder a repository skill's reference files are looked for.
const MAX_FILE_DEPTH: usize = 8;
/// The most entries one listing of a skill's reference files looks at.
const MAX_WALKED_ENTRIES: usize = 5000;

impl StepSkills {
  /// Offers the skills found under `folder`, a skills folder of `worktree` such as `<worktree>/.agents/skills`.
  pub fn add_repository(&mut self, folder: &Path, worktree: &Path) {
    let Ok(root) = fs::canonicalize(worktree).map(canonical::plain) else {
      self
        .left_out
        .push("the repository's skills: the worktree cannot be read".to_owned());
      return;
    };
    let mut search = Search {
      worktree,
      root,
      visited: HashSet::new(),
      offered: 0,
      bounded: false,
      skills: self,
    };
    search.folder(folder, 0);
  }
}

/// One search of a skills folder.
struct Search<'a> {
  /// The worktree as the runner names it, which every path found starts with.
  worktree: &'a Path,
  /// The worktree with its links resolved.
  root: PathBuf,
  /// The folders read, with their links resolved: a link loop reads each once.
  visited: HashSet<PathBuf>,
  offered: usize,
  /// The search stopped at one of its bounds, which the skills left out say once.
  bounded: bool,
  skills: &'a mut StepSkills,
}

impl Search<'_> {
  /// Looks for skills in the folder at `found`, `depth` below the skills folder.
  fn folder(&mut self, found: &Path, depth: usize) {
    let resolved = match fs::canonicalize(found) {
      Ok(resolved) => canonical::plain(resolved),
      Err(error) if error.kind() == io::ErrorKind::NotFound => return,
      Err(_) => return self.left_out(found, "cannot be read"),
    };
    if !resolved.starts_with(&self.root) {
      return self.left_out(found, "leads out of the worktree");
    }
    if protected(&resolved, &self.root, true) {
      return self.left_out(found, "leads into the worktree's .git or .orchester");
    }
    if !self.visited.insert(resolved.clone()) {
      return;
    }
    if depth > MAX_SEARCH_DEPTH || self.visited.len() > MAX_SEARCHED_FOLDERS {
      return self.stop();
    }
    let Ok(entries) = fs::read_dir(&resolved) else {
      return self.left_out(found, "cannot be read");
    };
    // An entry that cannot be read names nothing to look at.
    let mut names: Vec<OsString> = entries.flatten().map(|entry| entry.file_name()).collect();
    names.sort();
    for name in names {
      let child = found.join(&name);
      // A `.git` entry is the repository's own, never a skill's: it is passed over without a word.
      if protected(&resolved.join(&name), &resolved, false) {
        continue;
      }
      if name == SKILL_FILE {
        self.document(&child, &resolved);
      } else if fs::metadata(resolved.join(&name)).is_ok_and(|metadata| metadata.is_dir()) {
        self.folder(&child, depth + 1);
      }
    }
  }

  /// Offers the skill whose `SKILL.md` is at `found`, in the folder `folder` (resolved).
  fn document(&mut self, found: &Path, folder: &Path) {
    if self.offered >= MAX_REPOSITORY_SKILLS {
      return self.stop();
    }
    let Ok(resolved) = fs::canonicalize(found).map(canonical::plain) else {
      return self.left_out(found, "cannot be read");
    };
    let mut skill = OfferedSkill {
      name: String::new(),
      description: String::new(),
      source: SkillSource::Repository,
      folder: folder.to_path_buf(),
      listed: None,
      worktree: Some(self.root.clone()),
    };
    if !belongs(&resolved, &skill) || !is_regular_file(&resolved) {
      return self.left_out(found, "is not a file of its folder in the worktree");
    }
    let mut header = Vec::new();
    let read = File::open(&resolved).and_then(|file| file.take(MAX_HEADER_BYTES).read_to_end(&mut header));
    if read.is_err() {
      return self.left_out(found, "cannot be read");
    }
    let markdown = frontmatter::parse(&String::from_utf8_lossy(&header));
    let folder_name = found
      .parent()
      .and_then(Path::file_name)
      .map(|name| name.to_string_lossy().into_owned());
    match markdown.name.or(folder_name) {
      Some(name) if valid_name(&name) => skill.name = name,
      _ => return self.left_out(found, "its name is not one line of plain text"),
    }
    skill.description = markdown.description.unwrap_or_default();
    self.offered += 1;
    let shown = shown_in_worktree(found, self.worktree);
    self.skills.offer(skill, &shown);
  }

  /// Notes once that the search stopped at its bounds: what lies past them is not looked at.
  fn stop(&mut self) {
    if !self.bounded {
      self.bounded = true;
      let note = format!(
        "the search for the repository's skills stopped at its bounds ({MAX_SEARCH_DEPTH} folders deep, \
         {MAX_SEARCHED_FOLDERS} folders, {MAX_REPOSITORY_SKILLS} skills)"
      );
      self.skills.left_out.push(note);
    }
  }

  fn left_out(&mut self, found: &Path, why: &str) {
    let shown = shown_in_worktree(found, self.worktree);
    self.skills.left_out.push(format!("{shown}: {why}"));
  }
}

/// Whether `name` is one line of plain text a skill may be called by.
fn valid_name(name: &str) -> bool {
  !name.is_empty()
    && name.trim() == name
    && name.chars().count() <= MAX_NAME_CHARS
    && !name.chars().any(char::is_control)
}

/// The reference files of the repository skill `skill`, as paths from its folder with `/`: at most
/// [`MAX_LISTED_FILES`], and how many more there are.
pub(super) fn folder_files(skill: &OfferedSkill) -> (Vec<String>, usize) {
  // The skill's own folder counts as read: a link back to it lists nothing twice.
  let mut walk = FileWalk {
    skill,
    found: Vec::new(),
    visited: HashSet::from([skill.folder.clone()]),
    walked: 0,
  };
  walk.folder(&skill.folder, 0);
  let mut found = walk.found;
  found.sort();
  let more = found.len().saturating_sub(MAX_LISTED_FILES);
  found.truncate(MAX_LISTED_FILES);
  (found, more)
}

/// One listing of a repository skill's reference files. An entry that cannot be read or resolved is left out of the
/// list: the list only helps, and `read_skill` answers for each file with its own error.
struct FileWalk<'a> {
  skill: &'a OfferedSkill,
  found: Vec<String>,
  visited: HashSet<PathBuf>,
  walked: usize,
}

impl FileWalk<'_> {
  /// Collects the files of the folder at `current`, `depth` below the skill's folder, that [`belongs`] lets the
  /// skill read.
  fn folder(&mut self, current: &Path, depth: usize) {
    let Ok(entries) = fs::read_dir(current) else {
      return;
    };
    for entry in entries.flatten() {
      self.walked += 1;
      if self.walked > MAX_WALKED_ENTRIES {
        return;
      }
      let path = entry.path();
      let Ok(resolved) = fs::canonicalize(&path).map(canonical::plain) else {
        continue;
      };
      if resolved.is_dir() {
        let own_skill = resolved.join(SKILL_FILE).exists();
        let fresh = self.visited.insert(resolved.clone());
        if fresh && depth < MAX_FILE_DEPTH && !own_skill && belongs(&resolved, self.skill) {
          self.folder(&path, depth + 1);
        }
        continue;
      }
      let Ok(relative) = path.strip_prefix(&self.skill.folder) else {
        continue;
      };
      let relative = relative.to_string_lossy().replace('\\', "/");
      if belongs(&resolved, self.skill) && is_regular_file(&resolved) && SkillFilePath::new(&relative).is_ok() {
        self.found.push(relative);
      }
    }
  }
}
