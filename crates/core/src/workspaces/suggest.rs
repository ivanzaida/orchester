//! Suggested warm folders, from the git-ignored folders of the person's own checkout (read only, never written): those
//! over [`SUGGEST_OVER`] bytes, up to [`SUGGEST_DEPTH`] folders deep. Folders that differ only in the name of their
//! parent, such as `packages/web/node_modules/` and `packages/api/node_modules/`, are one pattern with `*` there
//! (`packages/*/node_modules/`), their sizes added. Orchester knows no languages: size is all it goes by.

use super::warm::WarmPattern;

/// Ignored folders smaller than this are not worth keeping: rebuilding them is quick.
pub const SUGGEST_OVER: u64 = 50 * 1024 * 1024;
/// How deep ignored folders are looked for.
pub const SUGGEST_DEPTH: usize = 3;

/// A git-ignored folder of the checkout and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgnoredFolder {
  /// Relative to the repository, with `/` between names; a closing `/` or not.
  pub path: String,
  pub bytes: u64,
}

/// A warm folder worth keeping: its pattern, how many folders of the checkout it names and their size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
  pub pattern: WarmPattern,
  pub folders: u32,
  pub bytes: u64,
}

/// The warm folders `ignored` suggests, largest first.
pub fn suggest(ignored: &[IgnoredFolder]) -> Vec<Suggestion> {
  let mut grouped: Vec<Suggestion> = Vec::new();
  let shallow = ignored
    .iter()
    .filter_map(|folder| WarmPattern::new(&folder.path).ok().map(|exact| (exact, folder.bytes)))
    .filter(|(exact, _)| exact.depth() <= SUGGEST_DEPTH);
  let folders: Vec<(WarmPattern, u64)> = shallow.collect();
  for (exact, bytes) in &folders {
    let pattern = parent_wildcard(exact)
      .filter(|wildcard| {
        folders
          .iter()
          .filter(|(other, _)| wildcard.matches(other.as_str()))
          .count()
          > 1
      })
      .unwrap_or_else(|| exact.clone());
    match grouped.iter_mut().find(|suggestion| suggestion.pattern == pattern) {
      Some(suggestion) => {
        suggestion.folders += 1;
        suggestion.bytes = suggestion.bytes.saturating_add(*bytes);
      }
      None => grouped.push(Suggestion {
        pattern,
        folders: 1,
        bytes: *bytes,
      }),
    }
  }
  grouped.retain(|suggestion| suggestion.bytes > SUGGEST_OVER);
  grouped.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.pattern.cmp(&b.pattern)));
  grouped
}

/// `exact` with its parent's name as `*` (`packages/web/node_modules/` gives `packages/*/node_modules/`); `None` for
/// a folder at the top, which has no parent to stand for.
fn parent_wildcard(exact: &WarmPattern) -> Option<WarmPattern> {
  let mut parts: Vec<&str> = exact.parts().collect();
  let parent = parts.len().checked_sub(2)?;
  parts[parent] = "*";
  WarmPattern::new(&parts.join("/")).ok()
}
