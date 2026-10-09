//! Orchester's `search_file`, `search_external_file` and the uses `find_references` lists, for agent CLIs: the files
//! under a folder that hold a term as literal text, in path order, each with its matching lines, at most a number of
//! files. In the worktree ripgrep searches with an argument vector Orchester builds (`--fixed-strings`, then
//! `--smart-case`: a term in lower case matches any case; or `--word-regexp --case-sensitive` for a name as a whole
//! word), which respects `.gitignore` and skips hidden entries; without it the folder is walked and each file searched
//! for the term. Outside the worktree the files are listed first (`rg --files`, or the walk), only those the caller
//! lets through are kept, and only those are read and searched: a file the caller refuses is never read, and the answer
//! says nothing about it.

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
  listing::{self, listed, walked},
  outcome,
  ripgrep::{self, Want},
  search::readable_text,
  walk::{MAX_WALKED, WALK_TIMEOUT},
};

/// Lines one file shows.
const MAX_LINES_PER_FILE: usize = 30;
/// Bytes of lines one answer shows, below what an answer holds.
const MAX_BYTES: usize = MAX_ANSWER_BYTES - 4 * 1024;
/// Characters a shown line keeps.
const MAX_LINE_CHARS: usize = 400;
const NO_RIPGREP: &str =
  "[ripgrep was not found: the files were walked without .gitignore rules, hidden ones left out]";

/// How a term matches a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Matching {
  /// As text anywhere in the line; a term in lower case in any case.
  Text,
  /// As a whole word, in exactly its case: a name as code uses it.
  Word,
}

/// One file's matching lines.
#[derive(Debug)]
struct FileHits {
  path: String,
  lines: Vec<String>,
  /// More lines match than it shows.
  more: bool,
}

/// What a search found so far.
struct Found {
  limit: usize,
  files: Vec<FileHits>,
  matches: usize,
  bytes: usize,
  /// More files match than were listed.
  more_files: bool,
  /// The answer reached its size.
  full: bool,
  notes: Vec<String>,
}

impl Found {
  fn new(limit: u32) -> Self {
    Self {
      limit: usize::try_from(limit).unwrap_or(usize::MAX),
      files: Vec::new(),
      matches: 0,
      bytes: 0,
      more_files: false,
      full: false,
      notes: Vec::new(),
    }
  }

  /// Takes line `number` of `path` (as the answer shows it), which holds the term.
  fn line(&mut self, path: &str, number: u64, text: &str) -> Want {
    if self.files.last().is_none_or(|last| last.path != path) {
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
    outcome(self.term_in(&worktree, (input.term(), input.limit_files(), Matching::Text)))
  }

  /// The worktree's files that use `name` as a whole word, at most `limit` of them, for `find_references`.
  pub fn search_word(&self, name: &str, limit: u32) -> ToolOutcome {
    let worktree = self.scope.worktree().to_path_buf();
    outcome(self.term_in(&worktree, (name, limit, Matching::Word)))
  }

  /// Orchester's `search_external_file`: the files under `folder` (outside the worktree, checked and resolved by the
  /// caller) that hold `term`, at most `limit` of them. Only the files `keep` lets through are read at all.
  pub fn search_outside(&self, folder: &Path, (term, limit): (&str, u32), keep: &dyn Fn(&Path) -> bool) -> ToolOutcome {
    outcome(self.term_in_kept(folder, (term, limit), keep))
  }

  fn term_in(&self, folder: &Path, (term, limit, matching): (&str, u32, Matching)) -> Result<ToolOutcome, FileError> {
    if !folder.is_dir() {
      return Err(FileError::NotFolder(self.shown(folder)));
    }
    let mut found = Found::new(limit);
    match &self.ripgrep {
      Some(program) => self.term_with_ripgrep(program, folder, (term, matching), &mut found)?,
      None => {
        let listing = walked(folder, None)?;
        self.term_in_files(listing, (term, matching), &mut found);
        found.notes.push(NO_RIPGREP.to_owned());
      }
    }
    Ok(answer(&found))
  }

  /// The search outside the worktree: the files under `folder` listed, those `keep` refuses dropped unread, the rest
  /// searched for `term`.
  fn term_in_kept(
    &self,
    folder: &Path,
    (term, limit): (&str, u32),
    keep: &dyn Fn(&Path) -> bool,
  ) -> Result<ToolOutcome, FileError> {
    if !folder.is_dir() {
      return Err(FileError::NotFolder(self.shown(folder)));
    }
    let mut listing = match &self.ripgrep {
      Some(program) => listed(program, folder, None)?,
      None => walked(folder, None)?,
    };
    listing.paths.retain(|path| keep(path));
    let mut found = Found::new(limit);
    self.term_in_files(listing, (term, Matching::Text), &mut found);
    if self.ripgrep.is_none() {
      found.notes.push(NO_RIPGREP.to_owned());
    }
    Ok(answer(&found))
  }
}

/// The outcome of a search that found `found`.
fn answer(found: &Found) -> ToolOutcome {
  let files = found.files.iter().filter(|file| !file.lines.is_empty()).count();
  let activity = format!("{} matches in {files} files", found.matches);
  let detail = CallDetail::Searched {
    scope: None,
    matches: count(found.matches),
    files: count(files),
  };
  done(found.text(), activity, Vec::new(), detail)
}

impl FileTools {
  fn term_with_ripgrep(
    &self,
    program: &Path,
    folder: &Path,
    (term, matching): (&str, Matching),
    found: &mut Found,
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
    match matching {
      Matching::Text => args.push("--smart-case".to_owned()),
      Matching::Word => args.extend(["--word-regexp".to_owned(), "--case-sensitive".to_owned()]),
    }
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
  fn term_line(&self, line: &str, found: &mut Found) -> Want {
    let Some((path, rest)) = line.split_once('\0') else {
      return Want::More;
    };
    let Some((number, text)) = rest.split_once(':') else {
      return Want::More;
    };
    let Ok(number) = number.parse::<u64>() else {
      return Want::More;
    };
    found.line(&self.shown(&PathBuf::from(path)), number, text)
  }

  /// Each file of `listing`, in path order, read and searched for `term` as ripgrep would, until the limits or the
  /// deadline.
  fn term_in_files(&self, listing: listing::Found, (term, matching): (&str, Matching), found: &mut Found) {
    let deadline = Instant::now() + WALK_TIMEOUT;
    let (mut files, mut stopped) = (listing.paths, listing.more);
    files.sort();
    let any_case = matching == Matching::Text && !term.chars().any(char::is_uppercase);
    let fold = |text: &str| if any_case { text.to_lowercase() } else { text.to_owned() };
    let needle = fold(term);
    let holds = |line: &str| match matching {
      Matching::Text => fold(line).contains(&needle),
      Matching::Word => holds_word(line, term),
    };
    for file in files {
      if Instant::now() > deadline {
        stopped = true;
        break;
      }
      let Some(text) = readable_text(&file) else {
        continue;
      };
      let shown = self.shown(&file);
      for (number, line) in (1_u64..).zip(text.lines()) {
        if holds(line) && found.line(&shown, number, line) == Want::Enough {
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

/// Whether `line` holds `word` with no word character right before or after it, as ripgrep's `--word-regexp` reads it.
fn holds_word(line: &str, word: &str) -> bool {
  let is_word = |c: char| c.is_alphanumeric() || c == '_';
  line.match_indices(word).any(|(at, _)| {
    let before = line[..at].chars().next_back();
    let after = line[at + word.len()..].chars().next();
    !before.is_some_and(is_word) && !after.is_some_and(is_word)
  })
}
