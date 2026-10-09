//! Orchester's `search_file` and `search_external_file` for agent CLIs: the files under a folder that hold a term as
//! literal text, in path order, each with its matching lines, at most a number of files. ripgrep searches with an
//! argument vector Orchester builds (`--fixed-strings`, `--smart-case`: a term in lower case matches any case), which
//! respects `.gitignore` and skips hidden entries; without it the folder is walked and each file searched for the
//! term. A search outside the worktree keeps only the files its caller lets through, which the answer counts without
//! naming them.

use std::{
  path::{Path, PathBuf},
  time::Instant,
};

use orchester_core::{
  connection::stream::CallDetail,
  native::session::ToolOutcome,
  orchester_tools::{MAX_ANSWER_BYTES, SearchTerm},
};

use super::{
  FileError, FileTools, count, done,
  limits::cut_line,
  outcome,
  ripgrep::{self, Want},
  search::readable_text,
  walk::{self, MAX_WALKED, WALK_TIMEOUT},
};

/// Lines one file shows.
const MAX_LINES_PER_FILE: usize = 30;
/// Bytes of lines one answer shows, below what an answer holds.
const MAX_BYTES: usize = MAX_ANSWER_BYTES - 4 * 1024;
/// Characters a shown line keeps.
const MAX_LINE_CHARS: usize = 400;
const NO_RIPGREP: &str =
  "[ripgrep was not found: the files were walked without .gitignore rules, hidden ones left out]";

/// One file's matching lines.
#[derive(Debug)]
struct FileHits {
  path: String,
  lines: Vec<String>,
  /// More lines match than it shows.
  more: bool,
}

/// What a search found so far.
struct Found<'a> {
  limit: usize,
  keep: &'a dyn Fn(&Path) -> bool,
  files: Vec<FileHits>,
  /// The file whose lines are being read was left out by `keep`.
  skipping: Option<String>,
  left_out: usize,
  matches: usize,
  bytes: usize,
  /// More files match than were listed.
  more_files: bool,
  /// The answer reached its size.
  full: bool,
  notes: Vec<String>,
}

impl Found<'_> {
  /// Takes line `number` of `path` (as the answer shows it, `real` on disk), which holds the term.
  fn line(&mut self, (path, real): (&str, &Path), number: u64, text: &str) -> Want {
    if self.skipping.as_deref() == Some(path) {
      return Want::More;
    }
    if self.files.last().is_none_or(|last| last.path != path) {
      if !(self.keep)(real) {
        self.skipping = Some(path.to_owned());
        self.left_out += 1;
        return Want::More;
      }
      if self.files.len() == self.limit {
        self.more_files = true;
        return Want::Enough;
      }
      self.files.push(FileHits {
        path: path.to_owned(),
        lines: Vec::new(),
        more: false,
      });
    }
    let row = format!(
      "{number:>6}: {}",
      cut_line(text.trim_end_matches(['\n', '\r']), MAX_LINE_CHARS)
    );
    let bytes = self.bytes + row.len() + path.len() + 2;
    let Some(file) = self.files.last_mut() else {
      return Want::More;
    };
    if file.lines.len() == MAX_LINES_PER_FILE {
      file.more = true;
      return Want::More;
    }
    if bytes > MAX_BYTES {
      self.full = true;
      return Want::Enough;
    }
    self.bytes = bytes;
    self.matches += 1;
    file.lines.push(row);
    Want::More
  }

  /// The answer: each file with its lines, then what was cut or left out.
  fn text(&self) -> String {
    let mut text = String::new();
    for file in self.files.iter().filter(|file| !file.lines.is_empty()) {
      text.push_str(&file.path);
      text.push('\n');
      for line in &file.lines {
        text.push_str(line);
        text.push('\n');
      }
      if file.more {
        text.push_str("   [more lines of this file match]\n");
      }
    }
    if text.is_empty() {
      text.push_str("no matches\n");
    }
    if self.more_files {
      text.push_str(&format!(
        "[more files match: only the first {} are listed; raise limit_files or search a longer term]\n",
        self.limit
      ));
    }
    if self.full {
      text.push_str("[the answer is full: search a longer term or fewer files]\n");
    }
    if self.left_out > 0 {
      text.push_str(&format!(
        "[{} files with matches left out: credential locations or Orchester's own files]\n",
        self.left_out
      ));
    }
    for note in &self.notes {
      text.push_str(note);
      text.push('\n');
    }
    text
  }
}

impl FileTools {
  /// Orchester's `search_file`: the worktree's files that hold the call's term.
  pub fn search_term(&self, input: &SearchTerm) -> ToolOutcome {
    let worktree = self.scope.worktree().to_path_buf();
    outcome(self.term_in(&worktree, (input.term(), input.limit_files()), &|_| true))
  }

  /// Orchester's `search_external_file`: the files under `folder` (outside the worktree, checked and resolved by the
  /// caller) that hold `term`, at most `limit` of them, keeping only those `keep` lets through.
  pub fn search_outside(&self, folder: &Path, (term, limit): (&str, u32), keep: &dyn Fn(&Path) -> bool) -> ToolOutcome {
    outcome(self.term_in(folder, (term, limit), keep))
  }

  fn term_in(
    &self,
    folder: &Path,
    (term, limit): (&str, u32),
    keep: &dyn Fn(&Path) -> bool,
  ) -> Result<ToolOutcome, FileError> {
    if !folder.is_dir() {
      return Err(FileError::NotFolder(self.shown(folder)));
    }
    let mut found = Found {
      limit: usize::try_from(limit).unwrap_or(usize::MAX),
      keep,
      files: Vec::new(),
      skipping: None,
      left_out: 0,
      matches: 0,
      bytes: 0,
      more_files: false,
      full: false,
      notes: Vec::new(),
    };
    match &self.ripgrep {
      Some(program) => self.term_with_ripgrep(program, folder, term, &mut found)?,
      None => {
        self.term_walking(folder, term, &mut found);
        found.notes.push(NO_RIPGREP.to_owned());
      }
    }
    let files = found.files.iter().filter(|file| !file.lines.is_empty()).count();
    let activity = format!("{} matches in {files} files", found.matches);
    let detail = CallDetail::Searched {
      scope: None,
      matches: count(found.matches),
      files: count(files),
    };
    Ok(done(found.text(), activity, Vec::new(), detail))
  }

  fn term_with_ripgrep(
    &self,
    program: &Path,
    folder: &Path,
    term: &str,
    found: &mut Found<'_>,
  ) -> Result<(), FileError> {
    let per_file = (MAX_LINES_PER_FILE + 1).to_string();
    let columns = MAX_LINE_CHARS.to_string();
    let mut args: Vec<String> = [
      "--with-filename",
      "--line-number",
      "--null",
      "--no-heading",
      "--color",
      "never",
      "--fixed-strings",
      "--smart-case",
      "--sort",
      "path",
      "--max-count",
      &per_file,
      "--max-columns",
      &columns,
      "--max-columns-preview",
    ]
    .map(str::to_owned)
    .to_vec();
    args.extend([
      "--regexp".to_owned(),
      term.to_owned(),
      "--".to_owned(),
      folder.display().to_string(),
    ]);
    let finished = ripgrep::run(program, &args, folder, |line| self.term_line(line, found))?;
    if finished.code == Some(2) {
      if found.matches == 0 {
        return Err(FileError::Ripgrep(finished.stderr));
      }
      let first = finished.stderr.lines().next().unwrap_or_default();
      found.notes.push(format!(
        "[ripgrep reported errors, so some files may be missing: {first}]"
      ));
    }
    Ok(())
  }

  /// One line of ripgrep's output, `path\0<line>:<text>`.
  fn term_line(&self, line: &str, found: &mut Found<'_>) -> Want {
    let Some((path, rest)) = line.split_once('\0') else {
      return Want::More;
    };
    let Some((number, text)) = rest.split_once(':') else {
      return Want::More;
    };
    let Ok(number) = number.parse::<u64>() else {
      return Want::More;
    };
    let real = PathBuf::from(path);
    found.line((&self.shown(&real), &real), number, text)
  }

  /// The search without ripgrep: each file under `folder`, in path order, searched for `term` as `--smart-case`
  /// would.
  fn term_walking(&self, folder: &Path, term: &str, found: &mut Found<'_>) {
    let deadline = Instant::now() + WALK_TIMEOUT;
    let (files, stopped) = walk::files(folder, deadline);
    let any_case = !term.chars().any(char::is_uppercase);
    let fold = |text: &str| if any_case { text.to_lowercase() } else { text.to_owned() };
    let needle = fold(term);
    for file in files {
      if Instant::now() > deadline {
        break;
      }
      let Some(text) = readable_text(&file) else {
        continue;
      };
      let shown = self.shown(&file);
      for (number, line) in (1_u64..).zip(text.lines()) {
        if fold(line).contains(&needle) && found.line((&shown, &file), number, line) == Want::Enough {
          return;
        }
      }
    }
    if stopped {
      found.notes.push(format!(
        "[the search stopped early (more than {MAX_WALKED} files or {} s); search a folder further down]",
        WALK_TIMEOUT.as_secs()
      ));
    }
  }
}
