//! The folder a `cd` with a glob goes to (`cd ~/.cargo/registry/src/*/serde-1.0.0`): bash expands the
//! glob first, and `cd` goes to the one folder it matches. Each component with `*` or `?` is matched
//! against what its folder holds now, case-sensitively as bash does, a leading `.` only by a pattern
//! that starts with one; a component with `[` is not read. When a component matches no folder or more
//! than one, `cd` fails or is refused in bash, and where the parts after it run cannot be told here.

use std::path::{Component, Path, PathBuf};

/// The one existing path `pattern` (absolute) matches; `None` when it matches none or several, or
/// holds a bracket expression.
pub(super) fn single_match(pattern: &Path) -> Option<PathBuf> {
  let mut found = PathBuf::new();
  for component in pattern.components() {
    let Component::Normal(name) = component else {
      found.push(component.as_os_str());
      continue;
    };
    let name = name.to_str()?;
    if name.contains('[') {
      return None;
    }
    if !name.contains(['*', '?']) {
      found.push(name);
      continue;
    }
    let mut matches = std::fs::read_dir(&found)
      .ok()?
      .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
      .filter(|entry| (name.starts_with('.') || !entry.starts_with('.')) && matches(name, entry));
    let only = matches.next()?;
    if matches.next().is_some() {
      return None;
    }
    found.push(only);
  }
  Some(found)
}

/// Whether `name` matches `pattern`, in which `*` stands for any characters and `?` for one.
fn matches(pattern: &str, name: &str) -> bool {
  let pattern: Vec<char> = pattern.chars().collect();
  let name: Vec<char> = name.chars().collect();
  // `reach[j]`: the pattern read so far matches the first `j` characters of the name.
  let mut reach = vec![false; name.len() + 1];
  reach[0] = true;
  for token in pattern {
    let mut next = vec![false; name.len() + 1];
    for j in 0..=name.len() {
      next[j] = match token {
        '*' => reach[j] || (j > 0 && next[j - 1]),
        '?' => j > 0 && reach[j - 1],
        c => j > 0 && reach[j - 1] && name[j - 1] == c,
      };
    }
    reach = next;
  }
  reach[name.len()]
}
