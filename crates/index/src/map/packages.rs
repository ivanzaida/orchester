//! Packages read from their manifests: Cargo crates (`Cargo.toml` with a `[package]`), npm packages
//! (`package.json`), Python projects (`pyproject.toml` with a `[project]` or Poetry table) and Go
//! modules (`go.mod`).

use std::collections::{BTreeMap, HashSet};

use serde_json::Value as Json;
use toml::{Table, Value as Toml};

/// The file names of the manifests a package is found by.
pub(super) const MANIFESTS: [&str; 4] = ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"];

pub(super) struct Package {
  /// Relative to the repository; empty for its root.
  pub(super) dir: String,
  pub(super) name: String,
  pub(super) ecosystem: &'static str,
  pub(super) description: Option<String>,
  /// Entry files (or scripts), described: `src/lib.rs (lib)`.
  pub(super) entries: Vec<String>,
  /// The folder holding the package's modules, relative to `dir`; the package's own folder when
  /// `None`.
  pub(super) source_root: Option<String>,
}

/// What a package reads besides its manifest: the tracked files and the other manifests.
pub(super) struct Context<'a> {
  pub(super) files: &'a HashSet<&'a str>,
  pub(super) manifests: &'a BTreeMap<String, String>,
}

impl Context<'_> {
  fn has(&self, dir: &str, path: &str) -> bool {
    self.files.contains(join(dir, path).as_str())
  }

  /// Whether the package in `dir` has tracked files under its folder `folder`.
  fn has_folder(&self, dir: &str, folder: &str) -> bool {
    let prefix = format!("{}/", join(dir, folder));
    self.files.iter().any(|file| file.starts_with(&prefix))
  }
}

/// The package the manifest at `path` describes; `None` when it describes none (a Cargo
/// workspace root, a `pyproject.toml` that only configures tools).
pub(super) fn read(path: &str, text: &str, context: &Context<'_>) -> Result<Option<Package>, String> {
  let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
  match file {
    "Cargo.toml" => cargo(dir, text, context),
    "package.json" => node(dir, text, context).map(Some),
    "pyproject.toml" => python(dir, text, context),
    "go.mod" => Ok(go(dir, text, context)),
    _ => Ok(None),
  }
}

fn cargo(dir: &str, text: &str, context: &Context<'_>) -> Result<Option<Package>, String> {
  let manifest: Table = toml::from_str(text).map_err(|error| error.message().to_owned())?;
  let Some(package) = manifest.get("package").and_then(Toml::as_table) else {
    return Ok(None);
  };
  let name = package
    .get("name")
    .and_then(Toml::as_str)
    .ok_or("the package has no name")?
    .to_owned();
  let description = match package.get("description") {
    Some(Toml::String(description)) => Some(description.clone()),
    Some(Toml::Table(inherited)) if inherited.get("workspace").and_then(Toml::as_bool) == Some(true) => {
      workspace_description(dir, context)
    }
    _ => None,
  };
  let lib = manifest
    .get("lib")
    .and_then(|lib| lib.get("path"))
    .and_then(Toml::as_str)
    .map(str::to_owned)
    .or_else(|| context.has(dir, "src/lib.rs").then(|| "src/lib.rs".to_owned()));
  let mut entries: Vec<String> = lib.iter().map(|lib| format!("{lib} (lib)")).collect();
  let bins = cargo_bins(dir, &manifest, &name, context);
  entries.extend(bins.iter().map(|(bin, path)| format!("{path} (bin {bin})")));
  let main = bins
    .iter()
    .find(|(_, path)| path == "src/main.rs")
    .map(|(_, path)| path);
  let source_root = lib
    .as_ref()
    .or(main)
    .and_then(|entry| entry.rsplit_once('/').map(|(folder, _)| folder.to_owned()))
    .or_else(|| context.has_folder(dir, "src").then(|| "src".to_owned()));
  Ok(Some(Package {
    dir: dir.to_owned(),
    name,
    ecosystem: "rust",
    description,
    entries,
    source_root,
  }))
}

/// The binaries of a crate, `(name, path)`: those its manifest lists, `src/main.rs` and
/// `src/bin/*.rs`.
fn cargo_bins(dir: &str, manifest: &Table, package: &str, context: &Context<'_>) -> Vec<(String, String)> {
  let mut bins: Vec<(String, String)> = manifest
    .get("bin")
    .and_then(Toml::as_array)
    .into_iter()
    .flatten()
    .filter_map(|bin| {
      let name = bin.get("name").and_then(Toml::as_str)?;
      let path = bin
        .get("path")
        .and_then(Toml::as_str)
        .map_or_else(|| format!("src/bin/{name}.rs"), str::to_owned);
      Some((name.to_owned(), path))
    })
    .collect();
  let listed = |bins: &[(String, String)], path: &str| bins.iter().any(|(_, listed)| listed == path);
  if context.has(dir, "src/main.rs") && !listed(&bins, "src/main.rs") {
    bins.push((package.to_owned(), "src/main.rs".to_owned()));
  }
  let prefix = format!("{}/", join(dir, "src/bin"));
  let mut automatic: Vec<&str> = context
    .files
    .iter()
    .filter_map(|file| file.strip_prefix(&prefix))
    .filter(|file| !file.contains('/') && file.ends_with(".rs"))
    .collect();
  automatic.sort_unstable();
  for file in automatic {
    let path = format!("src/bin/{file}");
    if !listed(&bins, &path) {
      bins.push((file.trim_end_matches(".rs").to_owned(), path));
    }
  }
  bins
}

/// `[workspace.package] description` of the nearest workspace above the crate in `dir`.
fn workspace_description(dir: &str, context: &Context<'_>) -> Option<String> {
  let mut folder = Some(dir);
  while let Some(current) = folder {
    let description = context
      .manifests
      .get(&join(current, "Cargo.toml"))
      // A workspace manifest that does not read gives no description; the map lists it as
      // unreadable where it describes it.
      .and_then(|text| toml::from_str::<Table>(text).ok())
      .and_then(|manifest| {
        let package = manifest.get("workspace")?.get("package")?;
        package.get("description")?.as_str().map(str::to_owned)
      });
    if description.is_some() {
      return description;
    }
    folder = (!current.is_empty()).then(|| current.rsplit_once('/').map_or("", |(parent, _)| parent));
  }
  None
}

fn node(dir: &str, text: &str, context: &Context<'_>) -> Result<Package, String> {
  let manifest: Json = serde_json::from_str(text).map_err(|error| error.to_string())?;
  let name = manifest["name"]
    .as_str()
    .map_or_else(|| folder_name(dir), str::to_owned);
  let mut entries = Vec::new();
  for field in ["main", "module", "types", "typings"] {
    if let Some(path) = manifest[field].as_str() {
      entries.push(format!("{} ({field})", relative(path)));
    }
  }
  match &manifest["bin"] {
    Json::String(path) => entries.push(format!("{} (bin {name})", relative(path))),
    Json::Object(bins) => entries.extend(
      bins
        .iter()
        .filter_map(|(bin, path)| Some(format!("{} (bin {bin})", relative(path.as_str()?)))),
    ),
    _ => {}
  }
  Ok(Package {
    dir: dir.to_owned(),
    name,
    ecosystem: "node",
    description: manifest["description"].as_str().map(str::to_owned),
    entries,
    source_root: context.has_folder(dir, "src").then(|| "src".to_owned()),
  })
}

fn python(dir: &str, text: &str, context: &Context<'_>) -> Result<Option<Package>, String> {
  let manifest: Table = toml::from_str(text).map_err(|error| error.message().to_owned())?;
  let project = manifest.get("project").or_else(|| manifest.get("tool")?.get("poetry"));
  let Some(name) = project.and_then(|project| project.get("name")).and_then(Toml::as_str) else {
    return Ok(None);
  };
  let scripts = project
    .and_then(|project| project.get("scripts"))
    .and_then(Toml::as_table)
    .into_iter()
    .flatten()
    .filter_map(|(script, target)| Some(format!("{} (script {script})", target.as_str()?)))
    .collect();
  let module = name.replace('-', "_");
  let source_root = ["src", module.as_str()]
    .into_iter()
    .find(|folder| context.has_folder(dir, folder))
    .map(str::to_owned);
  Ok(Some(Package {
    dir: dir.to_owned(),
    name: name.to_owned(),
    ecosystem: "python",
    description: project
      .and_then(|project| project.get("description"))
      .and_then(Toml::as_str)
      .map(str::to_owned),
    entries: scripts,
    source_root,
  }))
}

fn go(dir: &str, text: &str, context: &Context<'_>) -> Option<Package> {
  let name = text
    .lines()
    .find_map(|line| line.trim().strip_prefix("module "))?
    .trim()
    .to_owned();
  let prefix = format!("{}/", join(dir, "cmd"));
  let mut entries: Vec<String> = context
    .files
    .iter()
    .filter_map(|file| file.strip_prefix(&prefix))
    .filter(|file| file.ends_with("/main.go") && file.matches('/').count() == 1)
    .map(|file| format!("cmd/{file} (main)"))
    .collect();
  entries.sort_unstable();
  if context.has(dir, "main.go") {
    entries.insert(0, "main.go (main)".to_owned());
  }
  Some(Package {
    dir: dir.to_owned(),
    name,
    ecosystem: "go",
    description: None,
    entries,
    source_root: None,
  })
}

/// `path` in the package folder `dir`, relative to the repository.
pub(super) fn join(dir: &str, path: &str) -> String {
  if dir.is_empty() {
    path.to_owned()
  } else {
    format!("{dir}/{path}")
  }
}

fn relative(path: &str) -> &str {
  path.strip_prefix("./").unwrap_or(path)
}

fn folder_name(dir: &str) -> String {
  dir
    .rsplit('/')
    .next()
    .filter(|name| !name.is_empty())
    .unwrap_or(".")
    .to_owned()
}
