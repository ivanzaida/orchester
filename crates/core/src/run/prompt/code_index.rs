//! The prompt's part about the code index: where the index of the run's base revision is and how to
//! start with it, or how to search without ripgrep.

use std::fmt::Write as _;

use super::CodeIndexPrompt;

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
    } => {
      text.push_str("# Codebase index\n");
      // Writing to a String cannot fail.
      let _ = writeln!(
        text,
        "An index of this repository at the run's base revision {revision} is in {folder}: map.md (packages, \
         modules, entry points and test folders), tree.txt (every tracked file with its language and line count), \
         symbols.tsv (definitions by name, file and line), outline/<path>.md (the items of one file with their line \
         ranges) and docs.md (Markdown headings).\n\
         Start there instead of listing folders or reading whole files: read map.md once, then search symbols.tsv \
         and tree.txt with `rg -n`, and read only the lines you need. The codebase-map skill ({skill}) explains \
         each file and the searches.\n\
         Files changed since the base revision, by you or by earlier steps, may differ from the index: read those \
         files directly."
      );
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
