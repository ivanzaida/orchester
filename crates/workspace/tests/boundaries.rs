//! The crate holds no UI: lurq is an optional dependency for the empty markers of `src/inspect.rs`
//! only, and nothing else names it.

use std::{fs, path::Path, process::Command};

fn rust_sources(dir: &Path, out: &mut Vec<(String, String)>) {
  for entry in fs::read_dir(dir).expect("source directory is readable") {
    let path = entry.expect("source directory entry is readable").path();
    if path.is_dir() {
      rust_sources(&path, out);
    } else if path.extension().is_some_and(|ext| ext == "rs") {
      let text = fs::read_to_string(&path).expect("source file is UTF-8");
      out.push((path.display().to_string(), text));
    }
  }
}

/// The public Orchester Cloud lives in the `lurq.dev` domain. The build's default cloud address names that host, not
/// the UI toolkit (`docs/architecture/server.md`, row 59), so it is allowed once, in the one file that holds it.
const PUBLIC_CLOUD: &str = "\"https://orchester-cloud.lurq.dev\"";

#[test]
fn only_the_markers_name_lurq() {
  let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
  let mut sources = Vec::new();
  rust_sources(&src, &mut sources);
  let inspect = src.join("inspect.rs").display().to_string();
  let cloud = src.join("sharing").join("cloud").join("mod.rs").display().to_string();
  let naming: Vec<&String> = sources
    .iter()
    .filter(|(path, text)| {
      let text = if *path == cloud {
        text.replacen(PUBLIC_CLOUD, "", 1)
      } else {
        text.clone()
      };
      *path != inspect && text.contains("lurq")
    })
    .map(|(path, _)| path)
    .collect();
  assert!(naming.is_empty(), "sources that name lurq: {naming:?}");
}

/// The packages the crate builds with, on every target, with `features`: what Cargo resolves,
/// whatever a dependency is renamed to, which target it is for or what it re-exports.
fn resolved(features: &[&str]) -> Vec<String> {
  let mut tree = Command::new(env!("CARGO"));
  tree
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .args([
      "tree",
      "-p",
      "orchester-workspace",
      "--edges",
      "normal,build",
      "--target",
      "all",
    ])
    .args(["--prefix", "none", "--format", "{p}"]);
  if !features.is_empty() {
    tree.args(["--features", &features.join(",")]);
  }
  let output = tree.output().expect("cargo runs");
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
fn lurq_is_reachable_only_through_the_markers_feature() {
  let lurq = |packages: &[String]| packages.iter().any(|name| name.starts_with("lurq"));
  assert!(!lurq(&resolved(&[])), "lurq builds without `devtools-inspect`");
  assert!(
    lurq(&resolved(&["devtools-inspect"])),
    "`devtools-inspect` no longer brings the markers' lurq"
  );
}
