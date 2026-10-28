use orchester_core::workspaces::{IgnoredFolder, SUGGEST_OVER, suggest};

const MB: u64 = 1024 * 1024;

fn folder(path: &str, bytes: u64) -> IgnoredFolder {
  IgnoredFolder {
    path: path.to_owned(),
    bytes,
  }
}

#[test]
fn suggest_nested_folders_as_one_pattern_with_their_sizes_added() {
  let suggested = suggest(&[
    folder("target/", 38_000 * MB),
    folder("packages/web/node_modules/", 500 * MB),
    folder("packages/api/node_modules/", 300 * MB),
    folder("packages/cli/node_modules/", 60 * MB),
    folder(".venv/", 640 * MB),
  ]);
  let rows: Vec<(&str, u32, u64)> = suggested
    .iter()
    .map(|suggestion| (suggestion.pattern.as_str(), suggestion.folders, suggestion.bytes))
    .collect();
  assert_eq!(
    rows,
    [
      ("target/", 1, 38_000 * MB),
      ("packages/*/node_modules/", 3, 860 * MB),
      (".venv/", 1, 640 * MB),
    ]
  );
}

#[test]
fn suggest_leaves_out_small_and_deep_folders() {
  let suggested = suggest(&[
    folder(".tmp/", SUGGEST_OVER),
    folder("coverage/", 3 * MB),
    folder("a/b/c/d/", 900 * MB),
    folder("web/.next/", 120 * MB),
  ]);
  let patterns: Vec<&str> = suggested.iter().map(|suggestion| suggestion.pattern.as_str()).collect();
  assert_eq!(patterns, ["web/.next/"]);
}
