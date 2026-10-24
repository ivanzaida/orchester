//! Lists the bundled locale files for `src/i18n.rs`, and embeds the application icon in
//! `orchester.exe` on Windows, where Explorer, a taskbar pin and the Start menu shortcut take a
//! program's icon from its resources.
//!
//! On macOS a program has no embedded icon: it comes from the `.app` bundle (`CFBundleIconFile`; see
//! `tools/package-macos.sh`). There is nothing to build, so the icon step does nothing there and cannot
//! fail the build.
//! (`cfg(windows)` below is the build host; Orchester is built natively on each platform.)

#![allow(
  clippy::print_stdout,
  reason = "a build script talks to Cargo through standard output"
)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("cargo:rerun-if-changed=build.rs");
  locales::list()?;
  icon_resource::embed()
}

/// Writes `$OUT_DIR/locales.rs`: `LOCALES`, every `assets/locales/<locale>/<area>.json` included, sorted
/// by name. A new area or locale is a new file, with no list to edit beside it. Hidden entries and
/// editors' leftovers are passed over; anything else that is not a locale folder or an area's file stops
/// the build with its path.
mod locales {
  use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
  };

  const ROOT: &str = "assets/locales";

  pub fn list() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={ROOT}");
    let mut code = String::from("/// Every shipped locale with its areas' files, listed by `build.rs`.\n");
    code.push_str("pub const LOCALES: &[Locale] = &[\n");
    for (locale, folder) in entries(Path::new(ROOT))? {
      if !folder.is_dir() {
        return Err(
          format!(
            "{}: the locales folder holds only a folder per locale",
            folder.display()
          )
          .into(),
        );
      }
      writeln!(code, "  Locale {{ name: {locale:?}, areas: &[")?;
      for (file, path) in entries(&folder)? {
        let area = area(&file, &path)?;
        let included = format!("/{ROOT}/{locale}/{file}");
        writeln!(
          code,
          "    Area {{ name: {area:?}, contents: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {included:?})) }},"
        )?;
      }
      code.push_str("  ] },\n");
    }
    code.push_str("];\n");
    let out = Path::new(&env::var("OUT_DIR")?).join("locales.rs");
    fs::write(&out, code).map_err(|error| format!("{}: {error}", out.display()))?;
    Ok(())
  }

  /// The area the locale file `file` at `path` holds: a plain file `<area>.json`, the area in lowercase
  /// letters and `_`, as a key's first segment is.
  fn area<'a>(file: &'a str, path: &Path) -> Result<&'a str, String> {
    file
      .strip_suffix(".json")
      .filter(|area| !area.is_empty() && area.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'_'))
      .filter(|_| path.is_file())
      .ok_or_else(|| {
        format!(
          "{}: a locale folder holds only `<area>.json` files, the area in lowercase letters and `_`",
          path.display()
        )
      })
  }

  /// The entries of `folder` by name, sorted so the list does not depend on the file system's order,
  /// without hidden entries and editors' leftovers.
  fn entries(folder: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let unreadable = |error: std::io::Error| format!("{}: {error}", folder.display());
    let mut entries = Vec::new();
    for entry in fs::read_dir(folder).map_err(unreadable)? {
      let path = entry.map_err(unreadable)?.path();
      let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{}: the name is not UTF-8", path.display()))?
        .to_owned();
      if !is_leftover(&name) {
        entries.push((name, path));
      }
    }
    entries.sort();
    Ok(entries)
  }

  /// A hidden entry (`.DS_Store`, vim's `.shell.json.swp`) or an editor's backup or autosave
  /// (`shell.json~`, `#shell.json#`, `shell.json.bak`).
  fn is_leftover(name: &str) -> bool {
    name.starts_with(['.', '#'])
      || [".bak", ".swo", ".swp", ".tmp", "~"]
        .iter()
        .any(|end| name.ends_with(end))
  }
}

#[cfg(windows)]
mod icon_resource {
  const RESOURCE_SCRIPT: &str = "assets/icon/orchester.rc";
  const ICON: &str = "assets/icon/orchester.ico";

  pub fn embed() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={RESOURCE_SCRIPT}");
    println!("cargo:rerun-if-changed={ICON}");
    // Required rather than optional: a Windows build without its icon would look like success.
    // The MSVC toolchain installs the Windows SDK, which has the resource compiler.
    embed_resource::compile_for(RESOURCE_SCRIPT, ["orchester"], embed_resource::NONE).manifest_required()?;
    Ok(())
  }
}

#[cfg(not(windows))]
mod icon_resource {
  pub fn embed() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }
}
