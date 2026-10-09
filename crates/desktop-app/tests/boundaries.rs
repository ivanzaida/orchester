//! The app builds on `orchester-workspace`, the application layer both apps share, and never on the
//! deprecated `orchester-desktop`, whose screens it replaces: not directly, not through another crate,
//! not in its tests, examples or build script, with any of its features.

use std::process::Command;

/// The packages `orchester-app` builds with on every target, its tests' and build script's included:
/// what Cargo resolves, whatever a dependency is renamed to or which target it is for.
fn resolved() -> Vec<String> {
  let output = Command::new(env!("CARGO"))
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .args([
      "tree",
      "-p",
      "orchester-app",
      "--all-features",
      "--edges",
      "normal,build,dev",
      "--target",
      "all",
      "--prefix",
      "none",
      "--format",
      "{p}",
    ])
    .output()
    .expect("cargo runs");
  assert!(
    output.status.success(),
    "cargo tree failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout)
    .expect("cargo tree prints UTF-8")
    .lines()
    .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
    .collect()
}

#[test]
fn boundaries_the_app_uses_the_workspace_and_never_the_deprecated_desktop() {
  let packages = resolved();
  assert!(
    packages.iter().any(|name| name == "orchester-workspace"),
    "the app builds on the shared application layer: {packages:?}"
  );
  assert!(
    !packages.iter().any(|name| name == "orchester-desktop"),
    "orchester-desktop is reachable from orchester-app"
  );
}
