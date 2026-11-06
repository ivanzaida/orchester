//! A `Cargo.toml`'s binary targets as proposals: each `[[bin]]`, and unless `autobins = false` the ones Cargo finds
//! itself (`src/main.rs`, `src/bin/<name>.rs`, `src/bin/<name>/main.rs`), becomes a service that runs it with
//! `cargo run --bin <name>` in the package's folder, with the features it requires. A workspace's members are read as
//! their own manifests; a workspace manifest without a `[package]` has no binaries of its own. Nothing is built or
//! asked of cargo while reading. Ready once its port accepts connections, after a build that may take a while.

use orchester_core::service::{Readiness, ReadyCheck, ReadyTimeout};
use toml::{Table, Value};

use super::{Draft, Reading, Source, checkout::Checkout, folder_of, join, local};
use crate::messages::Message;

/// `cargo run` builds the binary first.
const TIMEOUT_SECS: u64 = 600;

/// A binary target and the key of the manifest, or the file, it was found by.
struct Binary {
  name: String,
  key: String,
  features: Vec<String>,
}

/// Proposes a service for each binary target of the `Cargo.toml` file `file`.
pub(super) fn read(checkout: &Checkout, file: &str, reading: &mut Reading) {
  let manifest = match checkout.read(file).and_then(|text| {
    text
      .parse::<Table>()
      .map_err(|error| Message::new("services.import.unreadable.toml").with("reason", error.message().to_owned()))
  }) {
    Ok(manifest) => manifest,
    Err(reason) => return reading.unreadable(file, reason),
  };
  let Some(package) = manifest.get("package").and_then(Value::as_table) else {
    return;
  };
  let Some(package_name) = package.get("name").and_then(Value::as_str) else {
    return reading.unreadable(file, Message::new("services.import.unreadable.cargoName"));
  };
  let folder = folder_of(file);
  let mut binaries = declared(&manifest, file, reading);
  if package.get("autobins").and_then(Value::as_bool) != Some(false) {
    for found in discovered(checkout, folder, package_name) {
      if !binaries.iter().any(|binary| binary.name == found.name) {
        binaries.push(found);
      }
    }
  }
  let readiness = Readiness::new(
    ReadyCheck::PortOpen,
    ReadyTimeout::from_secs(TIMEOUT_SECS).expect("ten minutes is a valid timeout"),
  );
  for binary in binaries {
    let source = Source {
      file: file.to_owned(),
      key: binary.key.clone(),
    };
    let name = reading.names.service(&binary.name, "app");
    let features = binary.features.join(",");
    let mut arguments = vec!["run", "--bin", binary.name.as_str()];
    if !features.is_empty() {
      arguments.extend(["--features", features.as_str()]);
    }
    match local::service((name, &binary.key), ("cargo", &arguments), folder, readiness.clone()) {
      Ok((service, unmapped)) => reading.propose(Draft::Service(Box::new(service)), source, unmapped),
      Err(reason) => reading.skip(source, reason),
    }
  }
}

/// The `[[bin]]` targets, by name; one without a name is passed over with why.
fn declared(manifest: &Table, file: &str, reading: &mut Reading) -> Vec<Binary> {
  let targets = manifest
    .get("bin")
    .and_then(Value::as_array)
    .map(Vec::as_slice)
    .unwrap_or_default();
  let mut binaries = Vec::new();
  for (index, target) in targets.iter().enumerate() {
    let Some(name) = target.get("name").and_then(Value::as_str) else {
      reading.skip(
        Source {
          file: file.to_owned(),
          key: format!("bin[{index}]"),
        },
        Message::new("services.import.skipped.binWithoutName"),
      );
      continue;
    };
    let features = target
      .get("required-features")
      .and_then(Value::as_array)
      .map(|features| features.iter().filter_map(Value::as_str).map(str::to_owned).collect())
      .unwrap_or_default();
    binaries.push(Binary {
      name: name.to_owned(),
      key: format!("bin.{name}"),
      features,
    });
  }
  binaries
}

/// The binaries Cargo finds in the package's folder `folder` without a `[[bin]]`, keyed by their files there.
fn discovered(checkout: &Checkout, folder: &str, package: &str) -> Vec<Binary> {
  let found = |name: &str, key: String| Binary {
    name: name.to_owned(),
    key,
    features: Vec::new(),
  };
  let mut binaries = Vec::new();
  if checkout.is_file(&join(folder, "src/main.rs")) {
    binaries.push(found(package, "src/main.rs".to_owned()));
  }
  let bin = join(folder, "src/bin");
  for file in checkout.files_in(&bin) {
    if let Some(name) = file.strip_suffix(".rs") {
      binaries.push(found(name, format!("src/bin/{file}")));
    }
  }
  for name in checkout.folders_in(&bin) {
    if checkout.is_file(&join(&bin, &format!("{name}/main.rs"))) {
      binaries.push(found(&name, format!("src/bin/{name}/main.rs")));
    }
  }
  binaries
}
