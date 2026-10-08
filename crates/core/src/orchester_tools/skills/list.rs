//! `list_skills`'s answer: each skill with its source, description and reference files, then the skills left out.
//! A step may offer many skills with long descriptions, so the answer keeps within [`MAX_LIST_CHARS`]: a description
//! past [`MAX_DESCRIPTION_CHARS`] ends with `…`, a skill's reference files past [`MAX_FILES_CHARS`] are counted, a
//! skill whose entry no longer fits is named alone, and one whose name no longer fits is counted, as are left-out
//! notes. A note at the end says what was cut and that `read_skill` reads a skill whole.

use std::fmt::Write;

use super::{OfferedSkill, StepSkills, repository};

/// The most characters of the list's entries; the notes on what it cut come after them.
const MAX_LIST_CHARS: usize = 12_000;
/// The most characters of one skill's description in the list, its `…` included.
const MAX_DESCRIPTION_CHARS: usize = 300;
/// The most characters of one skill's reference file names, as far as whole names fit; the rest are counted.
const MAX_FILES_CHARS: usize = 600;
/// Ends a description cut short.
const CUT: char = '…';
const HEADER: &str = "Read a skill with read_skill and its name; a reference file with its file too.\n";

impl StepSkills {
  /// Each skill with its source, description and reference files, then the skills left out, within the list's bounds.
  pub(super) fn list(&self) -> String {
    let mut list = List::default();
    list.push(HEADER);
    for skill in &self.skills {
      list.skill(skill);
    }
    list.left_out(&self.left_out);
    list.finish()
  }
}

/// A `list_skills` answer being written, and what it cut.
#[derive(Default)]
struct List {
  text: String,
  /// The characters of `text`.
  chars: usize,
  /// Skills listed with their description or reference files cut short.
  shortened: usize,
  /// Skills named without their description and reference files.
  named_only: usize,
  /// Skills not named.
  unlisted: usize,
  /// Left-out notes not shown.
  unlisted_left_out: usize,
}

impl List {
  /// Adds `entry` when it fits in the list's bounds.
  fn push(&mut self, entry: &str) -> bool {
    let chars = entry.chars().count();
    if self.chars + chars > MAX_LIST_CHARS {
      return false;
    }
    self.text.push_str(entry);
    self.chars += chars;
    true
  }

  /// Adds `skill` whole, or named alone when that no longer fits, or counts it.
  fn skill(&mut self, skill: &OfferedSkill) {
    let (files, more) = match &skill.listed {
      Some(listed) => (listed.iter().map(|file| file.as_str().to_owned()).collect(), 0),
      None => repository::folder_files(skill),
    };
    let (description, description_cut) = description(&skill.description);
    let (files, files_cut) = files_line(&files, more);
    let (name, source) = (&skill.name, skill.source.as_str());
    if self.push(&format!(
      "- {name} ({source}): {description}\n  reference files: {files}\n"
    )) {
      self.shortened += usize::from(description_cut || files_cut);
    } else if self.push(&format!("- {name} ({source})\n")) {
      self.named_only += 1;
    } else {
      self.unlisted += 1;
    }
  }

  /// Adds the skills left out, each with why, as far as they fit.
  fn left_out(&mut self, notes: &[String]) {
    if notes.is_empty() {
      return;
    }
    if !self.push("Left out:\n") {
      self.unlisted_left_out = notes.len();
      return;
    }
    for note in notes {
      if !self.push(&format!("- {note}\n")) {
        self.unlisted_left_out += 1;
      }
    }
  }

  /// The answer, with a note on each kind of cut it made.
  fn finish(mut self) -> String {
    // Writing to a String cannot fail.
    if self.shortened > 0 {
      let _ = writeln!(
        self.text,
        "Cut short to keep this answer small: {} {}; a description ends with {CUT}, and reference files that do not \
         fit are counted. read_skill with a skill's name reads it whole.",
        self.shortened,
        skills(self.shortened)
      );
    }
    if self.named_only > 0 {
      let _ = writeln!(
        self.text,
        "Named only, to keep this answer small: {} {}. read_skill with a skill's name reads it.",
        self.named_only,
        skills(self.named_only)
      );
    }
    if self.unlisted > 0 {
      let _ = writeln!(
        self.text,
        "Not listed, to keep this answer small: {} more {} offered.",
        self.unlisted,
        skills(self.unlisted)
      );
    }
    if self.unlisted_left_out > 0 {
      let _ = writeln!(
        self.text,
        "Not listed, to keep this answer small: {} more left out.",
        self.unlisted_left_out
      );
    }
    self.text
  }
}

/// "skill" after 1, "skills" after any other count.
const fn skills(count: usize) -> &'static str {
  if count == 1 { "skill" } else { "skills" }
}

/// `text` on one line, cut to [`MAX_DESCRIPTION_CHARS`] with a `…`; and whether it was cut.
fn description(text: &str) -> (String, bool) {
  let line = one_line(text);
  if line.chars().count() <= MAX_DESCRIPTION_CHARS {
    return (line, false);
  }
  let mut cut: String = line.chars().take(MAX_DESCRIPTION_CHARS - 1).collect();
  cut.push(CUT);
  (cut, true)
}

/// `files` by name, as far as whole names fit in [`MAX_FILES_CHARS`], then how many more there are with the `more`
/// not looked at; and whether any was left out.
fn files_line(files: &[String], more: usize) -> (String, bool) {
  if files.is_empty() && more == 0 {
    return ("none".to_owned(), false);
  }
  let (mut line, mut chars, mut shown) = (String::new(), 0, 0);
  for file in files {
    let separator = if shown == 0 { "" } else { ", " };
    let added = separator.len() + file.chars().count();
    if chars + added > MAX_FILES_CHARS {
      break;
    }
    line.push_str(separator);
    line.push_str(file);
    chars += added;
    shown += 1;
  }
  let left = files.len() - shown + more;
  if left > 0 {
    let separator = if shown == 0 { "" } else { ", " };
    // Writing to a String cannot fail.
    let _ = write!(line, "{separator}[{left} more not listed]");
  }
  (line, left > 0)
}

/// `text` on one line: its whitespace runs as one space, without control characters, so it cannot start a line of
/// its own in a list.
fn one_line(text: &str) -> String {
  text
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .chars()
    .filter(|c| !c.is_control())
    .collect()
}
