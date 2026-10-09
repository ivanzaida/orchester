//! The crate holds no UI: lurq is an optional dependency for the empty markers of `src/inspect.rs`
//! only, and nothing else names it.

use std::{fs, path::Path};

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

#[test]
fn only_the_markers_name_lurq() {
  let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
  let mut sources = Vec::new();
  rust_sources(&src, &mut sources);
  let inspect = src.join("inspect.rs").display().to_string();
  let naming: Vec<&String> = sources
    .iter()
    .filter(|(path, text)| *path != inspect && text.contains("lurq"))
    .map(|(path, _)| path)
    .collect();
  assert!(naming.is_empty(), "sources that name lurq: {naming:?}");
}

#[test]
fn lurq_is_optional_and_behind_the_markers_feature() {
  let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).expect("manifest");
  let manifest: toml::Table = manifest.parse().expect("manifest is TOML");
  let lurq = &manifest["dependencies"]["lurq"];
  assert_eq!(lurq.get("optional").and_then(toml::Value::as_bool), Some(true));
  let feature = manifest["features"]["devtools-inspect"]
    .as_array()
    .expect("the markers' feature lists what it enables");
  assert_eq!(feature, &vec![toml::Value::from("dep:lurq")]);
  for (name, value) in manifest["features"].as_table().expect("features") {
    let enables_lurq = value.as_array().is_some_and(|enabled| {
      enabled
        .iter()
        .any(|entry| entry.as_str().is_some_and(|entry| entry.contains("lurq")))
    });
    assert!(
      name == "devtools-inspect" || !enables_lurq,
      "feature `{name}` enables lurq"
    );
  }
}
