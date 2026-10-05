//! The prompt's part about the code index: where the index of the run's base revision is, its package and module
//! map and how to start with it, or how to search without ripgrep.

use std::fmt::Write as _;

use super::CodeIndexPrompt;

/// The most bytes of `map.md` a prompt carries, about 2000 tokens: the map of a workspace of a few hundred source
/// files fits once its module folders lose their contents.
const MAP_EXCERPT_LIMIT: usize = 8 * 1024;
/// Ends an excerpt that does not hold the whole map.
const MAP_CUT: &str = "- ... (the rest is in map.md)";

/// Appends what `code_index` tells the agent, under its own heading.
pub(super) fn push(text: &mut String, code_index: Option<CodeIndexPrompt<'_>>) {
  let Some(code_index) = code_index else {
    return;
  };
  if !text.is_empty() {
    if !text.ends_with('\n') {
      text.push('\n');
    }
    text.push('\n');
  }
  match code_index {
    CodeIndexPrompt::Given {
      revision,
      folder,
      skill,
      git_grep,
      map,
    } => {
      text.push_str("# Codebase index\n");
      // Writing to a String cannot fail.
      let _ = writeln!(
        text,
        "An index of this repository at the run's base revision {revision} is in {folder}: map.md (packages, \
         modules, entry points and test folders), tree.txt (every tracked file with its language and line count), \
         symbols.tsv (the code's types, functions and methods by name, file and line; constants and statics are in \
         symbols-consts.tsv, the definitions in test folders in symbols-tests.tsv), outline/<path>.md (the items of \
         one file with their line ranges) and docs.md (Markdown headings).\n\
         Start there instead of listing folders or reading whole files: find a definition with Orchester's \
         find_symbol tool, search with its search tool (a name's definitions come first) and list files with its \
         list_files tool, which all read this index; open the outline of a file before you read it, and read only \
         the lines you need. The orchester-codebase-index skill ({skill}) gives `rg` searches of these files, for \
         when Orchester's tools are not offered.\n\
         Files changed since the base revision, by you or by earlier steps, may differ from the index: read those \
         files directly."
      );
      if git_grep {
        text.push_str(
          "ripgrep (`rg`) is not available to you: search the index with `git grep --no-index -n` and the \
           repository with `git grep -n` on the narrowest path instead.\n",
        );
      }
      match MapExcerpt::of(map) {
        Some(excerpt) => excerpt.push(text),
        None => text.push_str(&format!(
          "Read map.md in {folder} first: it lists the packages, modules, entry points and test folders.\n"
        )),
      }
    }
    CodeIndexPrompt::WithoutRipgrep => {
      text.push_str("# Searching the repository\n");
      text.push_str(
        "ripgrep (`rg`) is not available to you. Search tracked files with `git grep -n` on the narrowest path \
         instead of listing folders or reading whole files, and read only the lines you need.\n",
      );
    }
  }
}

/// The package and module map of an index's `map.md`, in at most [`MAP_EXCERPT_LIMIT`] bytes.
struct MapExcerpt {
  /// Its lines, each ending in a line break.
  text: String,
  /// The module folders lost what they hold to fit.
  contents_omitted: bool,
}

impl MapExcerpt {
  /// The excerpt of `map` (the text of `map.md`): its sections without the title, blank lines, counts and the files
  /// beside a package's sources. A map too large for the bound loses what each module folder holds, then the lines
  /// that do not fit. `None` when `map` has no section.
  fn of(map: &str) -> Option<Self> {
    let first_section = map.find("\n## ")?;
    let mut lines: Vec<&str> = map[first_section + 1..]
      .lines()
      .filter(|line| !line.is_empty() && !line.starts_with("- files:") && !line.starts_with("- also:"))
      .collect();
    let contents_omitted = size(&lines) > MAP_EXCERPT_LIMIT;
    if contents_omitted {
      lines = lines.into_iter().map(without_contents).collect();
    }
    let fits = size(&lines) <= MAP_EXCERPT_LIMIT;
    let mut text = String::new();
    for line in lines {
      if !fits && text.len() + line.len() + MAP_CUT.len() + 2 > MAP_EXCERPT_LIMIT {
        text.push_str(MAP_CUT);
        text.push('\n');
        break;
      }
      text.push_str(line);
      text.push('\n');
    }
    Some(Self { text, contents_omitted })
  }

  /// Appends the excerpt to `prompt`, saying when the module file lists were left out, in a fence longer than any
  /// backtick run of the excerpt, so a path holding backticks cannot close it.
  fn push(&self, prompt: &mut String) {
    prompt.push_str("The packages and modules, from map.md");
    if self.contents_omitted {
      prompt.push_str(" (module file lists omitted; see map.md)");
    }
    let longest_run = self
      .text
      .split(|character| character != '`')
      .map(str::len)
      .max()
      .unwrap_or(0);
    let fence = "`".repeat(longest_run.max(2) + 1);
    prompt.push_str(&format!(":\n{fence}text\n{}{fence}\n", self.text));
  }
}

/// The bytes of `lines`, each with its line break.
fn size(lines: &[&str]) -> usize {
  lines.iter().map(|line| line.len() + 1).sum()
}

/// A module folder's line, `  - run/ (33 files, 5511 lines): again.rs, …`, without what it holds.
fn without_contents(line: &str) -> &str {
  match line.split_once("): ") {
    Some((folder, _)) if line.starts_with("  - ") => &line[..=folder.len()],
    _ => line,
  }
}
