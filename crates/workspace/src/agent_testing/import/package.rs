//! `package.json` scripts as proposals: each script named like `dev`, `start`, `preview` or `serve` (or `dev:web`)
//! becomes a service that runs it through the package's manager (`pnpm run dev`) in the package's folder. A package
//! of a workspace is read as its own manifest; the manager is the workspace's, told by the lockfile or the
//! `packageManager` of the nearest folder up that has one, else npm. The script itself is never run, and its text is
//! not taken apart: the manager reads it when the service starts. Ready once a line of its output shows a local URL,
//! as dev servers print where they listen.

use orchester_core::service::{LogPattern, Readiness, ReadyCheck, ReadyTimeout};
use serde_json::Value;

use super::{Draft, Reading, Source, checkout::Checkout, folder_of, join, local};
use crate::messages::Message;

/// The scripts that start an app, before any `:` in their names.
const SCRIPTS: &[&str] = &["dev", "start", "preview", "serve"];
/// The package managers that run a script as `<manager> run <script>`, with the files that tell each is used.
const MANAGERS: &[(&str, &[&str])] = &[
  ("pnpm", &["pnpm-lock.yaml", "pnpm-workspace.yaml"]),
  ("yarn", &["yarn.lock"]),
  ("bun", &["bun.lock", "bun.lockb"]),
  ("npm", &["package-lock.json", "npm-shrinkwrap.json"]),
];
/// A line a dev server prints once it listens, e.g. `Local: http://localhost:5173/`.
const LISTENING: &str = r"https?://(localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\])(:\d+)?";
/// A dev server may build first.
const TIMEOUT_SECS: u64 = 120;

/// One `package.json` read.
struct Manifest {
  file: String,
  value: Value,
}

/// Proposes a service for each app script of the `package.json` files `files`.
pub(super) fn read(checkout: &Checkout, files: &[String], reading: &mut Reading) {
  let mut manifests = Vec::new();
  for file in files {
    let parsed = checkout.read(file).and_then(|text| {
      serde_json::from_str::<Value>(&text)
        .map_err(|error| Message::new("services.import.unreadable.json").with("reason", error.to_string()))
    });
    match parsed {
      Ok(value) => manifests.push(Manifest {
        file: file.clone(),
        value,
      }),
      Err(reason) => reading.unreadable(file, reason),
    }
  }
  for manifest in &manifests {
    propose_scripts(checkout, manifest, &manifests, reading);
  }
}

fn propose_scripts(checkout: &Checkout, manifest: &Manifest, manifests: &[Manifest], reading: &mut Reading) {
  let Some(scripts) = manifest.value["scripts"].as_object() else {
    return;
  };
  let folder = folder_of(&manifest.file);
  let package = package_name(manifest);
  let manager = manager(checkout, folder, manifests);
  let readiness = Readiness::new(
    ReadyCheck::LogLine(LogPattern::new(LISTENING).expect("the pattern is a valid regular expression")),
    ReadyTimeout::from_secs(TIMEOUT_SECS).expect("two minutes is a valid timeout"),
  );
  for (script, _) in scripts.iter().filter(|(script, _)| starts_an_app(script)) {
    let key = format!("scripts.{script}");
    let source = Source {
      file: manifest.file.clone(),
      key: key.clone(),
    };
    let name = reading.names.service(&format!("{package}-{script}"), "app");
    let arguments = ["run", script.as_str()];
    match local::service((name, &key), (manager, &arguments), folder, readiness.clone()) {
      Ok((service, unmapped)) => reading.propose(Draft::Service(Box::new(service)), source, unmapped),
      Err(reason) => reading.skip(source, reason),
    }
  }
}

fn starts_an_app(script: &str) -> bool {
  let base = script.split(':').next().unwrap_or(script);
  SCRIPTS.contains(&base)
}

/// The package's `name` without its scope (`@kontur/web` is `web`), else its folder's name.
fn package_name(manifest: &Manifest) -> String {
  if let Some(name) = manifest.value["name"].as_str() {
    return name.rsplit('/').next().unwrap_or(name).to_owned();
  }
  let folder = folder_of(&manifest.file);
  folder.rsplit('/').next().unwrap_or(folder).to_owned()
}

/// The manager of the package in `folder`: the one the nearest folder up (itself included) uses by its lockfile or
/// `packageManager`, else npm.
fn manager(checkout: &Checkout, folder: &str, manifests: &[Manifest]) -> &'static str {
  let mut at = Some(folder);
  while let Some(current) = at {
    for (manager, files) in MANAGERS {
      if files.iter().any(|file| checkout.is_file(&join(current, file))) {
        return manager;
      }
    }
    let declared = manifests
      .iter()
      .find(|manifest| folder_of(&manifest.file) == current)
      .and_then(|manifest| manifest.value["packageManager"].as_str())
      .and_then(|declared| declared.split('@').next());
    if let Some(manager) = MANAGERS
      .iter()
      .map(|(manager, _)| *manager)
      .find(|manager| Some(*manager) == declared)
    {
      return manager;
    }
    at = (!current.is_empty()).then(|| folder_of(current));
  }
  "npm"
}
