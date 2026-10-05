//! `search` (`docs/architecture/native-agent.md` §4.3): ripgrep with an argument vector Orchester builds
//! (`--json`, `--max-count`, `--max-columns 500`, never `--pre` or `--search-zip`), at most 200 matches
//! or 64 KiB. Without ripgrep the pattern is searched as literal text in a folder walk, which the result
//! says. Matches read `path:line:text`, context lines `path-line-text`, as ripgrep prints them.

use std::{collections::BTreeSet, fs, path::Path};

use orchester_core::native::{session::ToolOutcome, tools::Search};
use serde_json::Value;

use super::{
  FileError, FileTools, done,
  glob::Glob,
  limits::cut_line,
  listing::relative,
  outcome,
  ripgrep::{self, Want},
  walk,
};

/// Matches one result shows.
const MAX_MATCHES: usize = 200;
/// Bytes one result shows.
const MAX_BYTES: usize = 64 * 1024;
/// Characters a shown line keeps.
const MAX_LINE_CHARS: usize = 500;
/// The largest file the search without ripgrep reads.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const NO_RIPGREP: &str = "[ripgrep was not found: the pattern was searched as literal text, without .gitignore rules]";

/// The lines found so far.
#[derive(Debug, Default)]
struct Hits {
  text: String,
  matches: usize,
  files: BTreeSet<String>,
  full: bool,
}

impl Hits {
  fn line(&mut self, path: &str, number: u64, line: &str, is_match: bool) -> Want {
    if self.full {
      return Want::Enough;
    }
    let separator = if is_match { ':' } else { '-' };
    let row = format!(
      "{path}{separator}{number}{separator}{}\n",
      cut_line(line.trim_end_matches(['\n', '\r']), MAX_LINE_CHARS)
    );
    if self.text.len() + row.len() > MAX_BYTES {
      self.full = true;
      return Want::Enough;
    }
    self.text.push_str(&row);
    if is_match {
      self.matches += 1;
      self.files.insert(path.to_owned());
      self.full = self.matches >= MAX_MATCHES;
    }
    if self.full { Want::Enough } else { Want::More }
  }
}

impl FileTools {
  pub fn search(&self, input: &Search) -> ToolOutcome {
    outcome(self.find(input))
  }

  fn find(&self, input: &Search) -> Result<ToolOutcome, FileError> {
    let target = self.scope.readable(input.path().unwrap_or("."))?;
    if !target.exists() {
      return Err(FileError::Missing(self.shown(&target)));
    }
    let mut hits = Hits::default();
    match &self.ripgrep {
      Some(program) => self.with_ripgrep(program, &target, input, &mut hits)?,
      None => self.literal(&target, input, &mut hits),
    }
    let mut text = if hits.matches == 0 {
      "no matches".to_owned()
    } else {
      std::mem::take(&mut hits.text)
    };
    if hits.full {
      text.push_str(&format!(
        "[truncated at {} matches or {MAX_BYTES} bytes; narrow the pattern, path or glob]",
        hits.matches
      ));
    }
    if self.ripgrep.is_none() {
      text.push('\n');
      text.push_str(NO_RIPGREP);
    }
    let activity = format!("{} matches in {} files", hits.matches, hits.files.len());
    Ok(done(text, activity, Vec::new()))
  }

  fn with_ripgrep(&self, program: &Path, target: &Path, input: &Search, hits: &mut Hits) -> Result<(), FileError> {
    let mut args: Vec<String> = ["--json", "--max-count", "200", "--max-columns", "500"]
      .map(str::to_owned)
      .to_vec();
    if input.case_insensitive() {
      args.push("--ignore-case".to_owned());
    }
    if input.context() > 0 {
      args.extend(["--context".to_owned(), input.context().to_string()]);
    }
    if let Some(glob) = input.glob() {
      args.extend(["--glob".to_owned(), glob.to_owned()]);
    }
    args.extend([
      "--regexp".to_owned(),
      input.pattern().to_owned(),
      "--".to_owned(),
      target.display().to_string(),
    ]);
    let folder = if target.is_dir() {
      target
    } else {
      target.parent().unwrap_or(target)
    };
    let finished = ripgrep::run(program, &args, folder, |line| self.json_line(line, hits))?;
    if finished.code == Some(2) && hits.matches == 0 {
      return Err(FileError::Ripgrep(finished.stderr));
    }
    Ok(())
  }

  /// One line of `rg --json`: a match or a context line goes to `hits`, anything else is skipped.
  fn json_line(&self, line: &str, hits: &mut Hits) -> Want {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
      return Want::More;
    };
    let is_match = match event["type"].as_str() {
      Some("match") => true,
      Some("context") => false,
      _ => return Want::More,
    };
    let data = &event["data"];
    let (Some(path), Some(number), Some(text)) = (
      data["path"]["text"].as_str(),
      data["line_number"].as_u64(),
      data["lines"]["text"].as_str(),
    ) else {
      return Want::More;
    };
    hits.line(&self.shown(Path::new(path)), number, text, is_match)
  }

  /// The search without ripgrep: `input`'s pattern as literal text in the files under `target`.
  fn literal(&self, target: &Path, input: &Search, hits: &mut Hits) {
    let (files, _) = walk::files(target);
    let folder = if target.is_dir() {
      target
    } else {
      target.parent().unwrap_or(target)
    };
    let glob = input.glob().map(Glob::new);
    let fold = |text: &str| {
      if input.case_insensitive() {
        text.to_lowercase()
      } else {
        text.to_owned()
      }
    };
    let needle = fold(input.pattern());
    for file in files {
      if glob
        .as_ref()
        .is_some_and(|glob| !glob.matches(&relative(folder, &file)))
      {
        continue;
      }
      let Some(text) = readable_text(&file) else {
        continue;
      };
      let lines: Vec<&str> = text.lines().collect();
      let found: Vec<usize> = (0..lines.len())
        .filter(|at| fold(lines[*at]).contains(&needle))
        .collect();
      if self.show(&file, &lines, &found, input.context(), hits) == Want::Enough {
        return;
      }
    }
  }

  /// Shows the lines `found` of `file` with `context` lines around each.
  fn show(&self, file: &Path, lines: &[&str], found: &[usize], context: u32, hits: &mut Hits) -> Want {
    let context = usize::try_from(context).unwrap_or(usize::MAX);
    let shown = self.shown(file);
    let mut next = 0;
    for at in found {
      let start = at.saturating_sub(context).max(next);
      let end = at.saturating_add(context).min(lines.len() - 1);
      for (line, text) in lines.iter().enumerate().take(end + 1).skip(start) {
        let number = u64::try_from(line + 1).unwrap_or(u64::MAX);
        if hits.line(&shown, number, text, found.contains(&line)) == Want::Enough {
          return Want::Enough;
        }
      }
      next = end + 1;
    }
    Want::More
  }
}

/// The text of `file`, unless it is large or binary.
fn readable_text(file: &Path) -> Option<String> {
  let metadata = fs::metadata(file).ok()?;
  if metadata.len() > MAX_FILE_BYTES {
    return None;
  }
  let bytes = fs::read(file).ok()?;
  if bytes.iter().take(8 * 1024).any(|byte| *byte == 0) {
    return None;
  }
  Some(String::from_utf8_lossy(&bytes).into_owned())
}
