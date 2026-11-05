//! Copying what a suite left into the run's storage: the files its artifact globs match under its working folder, and
//! the files its report attaches to a failed test (a Playwright screenshot or trace). Only files inside the worktree
//! are copied, never through a link that leads out of it, and within a number and a size, so a glob that matches a
//! whole build folder cannot fill the disk. A copy keeps its path in the worktree under the test run's `artifacts/`.

use std::{
  collections::BTreeMap,
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::{
  resource::{ArtifactGlob, TestSuite},
  test_report::{StoredArtifact, TestReport, test_name},
};

use super::store::shown;

/// The most files one test run copies.
const MAX_FILES: usize = 200;
/// The most bytes one test run copies.
const MAX_BYTES: u64 = 256 * 1024 * 1024;
/// The most entries a glob's search looks at.
const MAX_SEARCHED: usize = 50_000;

/// The copies of one test run, made into `into`.
pub(super) struct Copies {
  /// The worktree, resolved.
  worktree: PathBuf,
  /// The suite's working folder, as spelled in the worktree.
  folder: PathBuf,
  into: PathBuf,
  copied: BTreeMap<String, String>,
  bytes: u64,
  pub(super) stored: Vec<StoredArtifact>,
  pub(super) problems: Vec<String>,
}

impl Copies {
  /// Copies what `suite`, working in `worktree`, left after a run whose report is `report`, into `into`.
  pub(super) fn of(suite: &TestSuite, worktree: &Path, report: Option<&TestReport>, into: PathBuf) -> Self {
    let mut copies = Self {
      // A worktree that cannot be resolved holds no resolved file either, so then each copy says why it was not made.
      worktree: fs::canonicalize(worktree).unwrap_or_else(|_| worktree.to_path_buf()),
      folder: suite.folder_in(worktree),
      into,
      copied: BTreeMap::new(),
      bytes: 0,
      stored: Vec::new(),
      problems: Vec::new(),
    };
    for glob in suite.artifacts() {
      copies.glob(glob);
    }
    for case in report.into_iter().flat_map(TestReport::failures) {
      let name = test_name(case);
      for attachment in case.attachments() {
        copies.attached(&name, &attachment.path);
      }
    }
    copies
  }

  /// Copies the files `glob` matches under the working folder.
  fn glob(&mut self, glob: &ArtifactGlob) {
    let base = glob.base().iter().fold(self.folder.clone(), |path, part| path.join(part));
    let mut pending = vec![base];
    let mut searched = 0;
    while let Some(folder) = pending.pop() {
      let Ok(entries) = fs::read_dir(&folder) else {
        continue;
      };
      for entry in entries.flatten() {
        searched += 1;
        if searched > MAX_SEARCHED {
          self.problems.push(format!(
            "`{}` matches in a folder of more than {MAX_SEARCHED} entries; the search stopped there",
            glob.as_str()
          ));
          return;
        }
        let path = entry.path();
        // A link is never followed: it could lead out of the worktree or round in a circle.
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() && entry.file_name() != ".git" {
          pending.push(path);
        } else if kind.is_file()
          && let Some(relative) = relative_to(&path, &self.folder)
          && glob.matches(&relative)
        {
          self.copy(&path, None);
        }
      }
    }
  }

  /// Copies the file the report attached to the test `name` at `path`, relative to the working folder or absolute.
  fn attached(&mut self, name: &str, path: &str) {
    let file = Path::new(path);
    let file = if file.is_absolute() {
      file.to_path_buf()
    } else {
      self.folder.join(file)
    };
    self.copy(&file, Some(name));
  }

  /// Copies `file` into the test run's artifacts, at its path in the worktree, once; `test` names the test the report
  /// attached it to.
  fn copy(&mut self, file: &Path, test: Option<&str>) {
    let source = match self.inside(file) {
      Ok(source) => source,
      Err(problem) => {
        self.problems.push(problem);
        return;
      }
    };
    if let Some(stored) = self.copied.get(&source) {
      let stored = stored.clone();
      if test.is_some() {
        self.stored.push(artifact(source, stored, test));
      }
      return;
    }
    if self.copied.len() >= MAX_FILES {
      self.problems.push(format!("`{source}` was not copied: a test run keeps at most {MAX_FILES} files"));
      return;
    }
    let size = match fs::metadata(file) {
      Ok(metadata) => metadata.len(),
      Err(error) => {
        self.problems.push(format!("`{source}` could not be copied: {error}"));
        return;
      }
    };
    if self.bytes + size > MAX_BYTES {
      self.problems.push(format!(
        "`{source}` was not copied: a test run keeps at most {} MiB",
        MAX_BYTES / (1024 * 1024)
      ));
      return;
    }
    let target = source.split('/').fold(self.into.clone(), |path, part| path.join(part));
    match copied(file, &target) {
      Ok(()) => {
        self.bytes += size;
        let stored = shown(&target);
        self.copied.insert(source.clone(), stored.clone());
        self.stored.push(artifact(source, stored, test));
      }
      Err(error) => self.problems.push(format!("`{source}` could not be copied: {error}")),
    }
  }

  /// The path of `file` in the worktree, with `/`; why it is not copied when it is not a file inside the worktree.
  fn inside(&self, file: &Path) -> Result<String, String> {
    let named = shown(file);
    let resolved = fs::canonicalize(file).map_err(|error| format!("`{named}` could not be copied: {error}"))?;
    if !resolved.is_file() {
      return Err(format!("`{named}` is not a file"));
    }
    relative_to(&resolved, &self.worktree).ok_or_else(|| format!("`{named}` is outside the worktree, so it was not copied"))
  }
}

fn artifact(source: String, stored: String, test: Option<&str>) -> StoredArtifact {
  StoredArtifact {
    source,
    stored,
    test: test.map(str::to_owned),
  }
}

/// `path` relative to `base`, with `/` between its parts; `None` when it is not under `base` or not valid Unicode.
fn relative_to(path: &Path, base: &Path) -> Option<String> {
  let relative = path.strip_prefix(base).ok()?;
  let parts: Option<Vec<&str>> = relative.components().map(|part| part.as_os_str().to_str()).collect();
  parts.filter(|parts| !parts.is_empty()).map(|parts| parts.join("/"))
}

fn copied(file: &Path, target: &Path) -> io::Result<()> {
  if let Some(folder) = target.parent() {
    fs::create_dir_all(folder)?;
  }
  fs::copy(file, target).map(|_| ())
}
