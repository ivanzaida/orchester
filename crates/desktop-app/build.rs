//! Embeds the application icon in `orchester-app.exe` on Windows, where Explorer, a taskbar pin and
//! the Start menu shortcut take a program's icon from its resources.
//!
//! On macOS a program has no embedded icon: it comes from an `.app` bundle, which this app does not
//! have yet. There is nothing to build, so the script does nothing there and cannot fail the build.
//! (`cfg(windows)` below is the build host; Orchester is built natively on each platform.)

#![allow(
  clippy::print_stdout,
  reason = "a build script talks to Cargo through standard output"
)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("cargo:rerun-if-changed=build.rs");
  icon_resource::embed()
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
