//! Lists the bundled locale files for `src/i18n.rs`, and embeds the application icon in
//! `orchester-app.exe` on Windows, where Explorer, a taskbar pin and the Start menu shortcut take a
//! program's icon from its resources.
//!
//! On macOS a program has no embedded icon: it comes from an `.app` bundle, which this app does not
//! have yet. There is nothing to build, so the icon step does nothing there and cannot fail the build.
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
/// by name. A new area or locale is a new file, with no list to edit beside it.
mod locales {
  use std::{env, error::Error, fmt::Write as _, fs, path::Path};

  const ROOT: &str = "assets/locales";

  pub fn list() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={ROOT}");
    let mut code = String::from("/// Every shipped locale with its areas' files, listed by `build.rs`.\n");
    code.push_str("pub const LOCALES: &[Locale] = &[\n");
    for (locale, folder) in entries(Path::new(ROOT))? {
      writeln!(code, "  Locale {{ name: {locale:?}, areas: &[")?;
      for (file, path) in entries(&folder)? {
        let area = file
          .strip_suffix(".json")
          .ok_or_else(|| format!("{} is not a locale file (`<area>.json`)", path.display()))?;
        let included = format!("/{ROOT}/{locale}/{file}");
        writeln!(
          code,
          "    Area {{ name: {area:?}, contents: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {included:?})) }},"
        )?;
      }
      code.push_str("  ] },\n");
    }
    code.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR")?).join("locales.rs"), code)?;
    Ok(())
  }

  /// The entries of `folder` by name, sorted so the list does not depend on the file system's order.
  fn entries(folder: &Path) -> Result<Vec<(String, std::path::PathBuf)>, Box<dyn Error>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(folder)? {
      let path = entry?.path();
      let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no UTF-8 name", path.display()))?
        .to_owned();
      entries.push((name, path));
    }
    entries.sort();
    Ok(entries)
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
    embed_resource::compile_for(RESOURCE_SCRIPT, ["orchester-app"], embed_resource::NONE).manifest_required()?;
    Ok(())
  }
}

#[cfg(not(windows))]
mod icon_resource {
  pub fn embed() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }
}
