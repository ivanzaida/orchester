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
         Start there instead of listing folders or reading whole files: search symbols.tsv and tree.txt with \
         `rg -n`, open the outline of a file before you read it, and read only the lines you need. The \
         orchester-codebase-index skill ({skill}) gives these searches as commands for PowerShell and bash.\n\
         Files changed since the base revision, by you or by earlier steps, may differ from the index: read those \
         files directly."
      );
      if git_grep {
        text.push_str(
          "ripgrep (`rg`) is not available to you: search the index with `git grep --no-index -n` and the \
           repository with `git grep -n` on the narrowest path instead.\n",
        );
      }
      let excerpt = map_excerpt(map);
      if !excerpt.is_empty() {
        text.push_str(&format!("The packages and modules, from map.md:\n```text\n{excerpt}```\n"));
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

/// The package and module map of `map` (the text of `map.md`), each line ending in a line break, in at most
/// [`MAP_EXCERPT_LIMIT`] bytes: its sections without the title, blank lines, counts and the files beside a package's
/// sources. A map too large for that loses what each module folder holds, then the lines that do not fit.
fn map_excerpt(map: &str) -> String {
  let Some(first_section) = map.find("\n## ") else {
    return String::new();
  };
  let mut lines: Vec<&str> = map[first_section + 1..]
    .lines()
    .filter(|line| !line.is_empty() && !line.starts_with("- files:") && !line.starts_with("- also:"))
    .collect();
  if size(&lines) > MAP_EXCERPT_LIMIT {
    lines = lines.into_iter().map(without_contents).collect();
  }
  let fits = size(&lines) <= MAP_EXCERPT_LIMIT;
  let mut excerpt = String::new();
  for line in lines {
    if !fits && excerpt.len() + line.len() + MAP_CUT.len() + 2 > MAP_EXCERPT_LIMIT {
      excerpt.push_str(MAP_CUT);
      excerpt.push('\n');
      break;
    }
    excerpt.push_str(line);
    excerpt.push('\n');
  }
  excerpt
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
