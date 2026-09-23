//! Formatting of raw data for narrow places in the UI.

use std::path::{MAIN_SEPARATOR, Path};

/// `path` in at most `max_chars` characters where possible: the start and the last folders
/// stay, the middle becomes an ellipsis (`H:\…\work\kontur`). The last folder is always kept
/// whole, since it names the repository.
pub fn shorten_path(path: &Path, max_chars: usize) -> String {
  let full = path.display().to_string();
  if full.chars().count() <= max_chars {
    return full;
  }
  let parts: Vec<&str> = full.split(['\\', '/']).collect();
  let (Some(head), Some((last, middle))) = (parts.first(), parts[1..].split_last()) else {
    return full;
  };
  let mut tail = vec![*last];
  for part in middle.iter().rev() {
    let candidate = part.chars().count() + tail.iter().map(|kept| kept.chars().count() + 1).sum::<usize>();
    if head.chars().count() + 3 + candidate > max_chars {
      break;
    }
    tail.insert(0, part);
  }
  if tail.len() == parts.len() - 1 {
    return full;
  }
  let separator = full
    .chars()
    .find(|c| matches!(c, '\\' | '/'))
    .unwrap_or(MAIN_SEPARATOR)
    .to_string();
  format!("{head}{separator}…{separator}{}", tail.join(&separator))
}
