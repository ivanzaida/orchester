//! Finding ripgrep for agents (on the `PATH`, then bundled with the Codex CLI or Claude Code),
//! copying it into the project's tools folder by content, and the install plan, which is unavailable
//! without its installer.

use std::{
  env,
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
};

use orchester_exec::ripgrep::{
  self, Found, PROGRAM, Source,
  install::{InstallPlan, InstallUnavailable, Installer},
};

/// The Codex CLI package's target folder on this platform.
const CODEX_TARGET: &str = if cfg!(windows) {
  "x86_64-pc-windows-msvc"
} else if cfg!(target_arch = "aarch64") {
  "aarch64-apple-darwin"
} else {
  "x86_64-apple-darwin"
};
/// Claude Code's ripgrep folder on this platform.
const CLAUDE_PLATFORM: &str = if cfg!(windows) {
  "x64-win32"
} else if cfg!(target_arch = "aarch64") {
  "arm64-darwin"
} else {
  "x64-darwin"
};

fn file(path: &Path, content: &str) -> PathBuf {
  fs::create_dir_all(path.parent().expect("a folder")).expect("create folder");
  fs::write(path, content).expect("write");
  path.to_path_buf()
}

fn path_of(folders: &[&Path]) -> OsString {
  env::join_paths(folders).expect("joinable")
}

/// A global npm install of the Codex CLI in `prefix` with its platform package's ripgrep; the CLI's
/// script.
fn codex_install(prefix: &Path) -> (PathBuf, PathBuf) {
  let package = prefix.join("node_modules/@openai/codex");
  let script = file(&package.join("bin/codex.js"), "// codex");
  let rg = file(
    &package
      .join("node_modules/@openai/codex-platform/vendor")
      .join(CODEX_TARGET)
      .join("codex-path")
      .join(PROGRAM),
    "codex rg",
  );
  (script, rg)
}

#[test]
fn ripgrep_on_the_path_comes_before_a_bundled_one() {
  let dir = tempfile::tempdir().expect("temp");
  let (codex, _) = codex_install(&dir.path().join("npm"));
  let bin = dir.path().join("bin");
  let rg = file(&bin.join(PROGRAM), "path rg");
  let empty = dir.path().join("empty");
  fs::create_dir_all(&empty).expect("folder");

  let found = ripgrep::locate(Some(&path_of(&[&empty, &bin])), &[&codex]);

  assert_eq!(
    found,
    Some(Found {
      path: rg,
      source: Source::OnPath
    })
  );
}

#[test]
fn ripgrep_bundled_with_the_codex_cli_is_found_from_its_script() {
  let dir = tempfile::tempdir().expect("temp");
  let (codex, rg) = codex_install(&dir.path().join("npm"));

  let found = ripgrep::locate(Some(&path_of(&[dir.path()])), &[&codex]).expect("found");

  assert_eq!(found.source, Source::CodexCli);
  assert_eq!(
    fs::read_to_string(found.path).expect("read"),
    fs::read_to_string(rg).expect("read")
  );
}

#[test]
fn ripgrep_bundled_with_claude_code_is_found_from_its_script() {
  let dir = tempfile::tempdir().expect("temp");
  let package = dir.path().join("node_modules/@anthropic-ai/claude-code");
  let script = file(&package.join("cli.js"), "// claude");
  file(
    &package.join("vendor/ripgrep").join(CLAUDE_PLATFORM).join(PROGRAM),
    "claude rg",
  );

  let found = ripgrep::locate(None, &[&script]).expect("found");

  assert_eq!(found.source, Source::ClaudeCode);
  assert_eq!(fs::read_to_string(found.path).expect("read"), "claude rg");
}

#[test]
fn ripgrep_is_not_found_without_one() {
  let dir = tempfile::tempdir().expect("temp");
  let cli = file(&dir.path().join("native/claude"), "native");
  assert_eq!(ripgrep::locate(Some(&path_of(&[dir.path()])), &[&cli]), None);
}

#[test]
fn ripgrep_is_copied_once_per_content_into_the_tools_folder() {
  let dir = tempfile::tempdir().expect("temp");
  let tools = dir.path().join("repo/.orchester/tools");
  let first = Found {
    path: file(&dir.path().join("a").join(PROGRAM), "ripgrep 14"),
    source: Source::OnPath,
  };

  let folder = ripgrep::provide(&first, &tools).expect("copied");
  assert!(folder.starts_with(&tools));
  assert_eq!(fs::read_to_string(folder.join(PROGRAM)).expect("copy"), "ripgrep 14");
  assert_eq!(ripgrep::provide(&first, &tools).expect("again"), folder);

  let newer = Found {
    path: file(&dir.path().join("b").join(PROGRAM), "ripgrep 15"),
    source: Source::CodexCli,
  };
  let other = ripgrep::provide(&newer, &tools).expect("copied");
  assert_ne!(other, folder);
  let entries: Vec<_> = fs::read_dir(&tools).expect("read").collect();
  assert_eq!(entries.len(), 2, "no staging folder is left");
}

#[test]
fn installing_ripgrep_needs_the_platform_installer() {
  let dir = tempfile::tempdir().expect("temp");
  let plan = InstallPlan::find(Some(&path_of(&[dir.path()])));
  match Installer::of_platform() {
    Some(installer) => assert_eq!(plan, Err(InstallUnavailable::InstallerMissing { installer })),
    None => assert_eq!(plan, Err(InstallUnavailable::Unsupported)),
  }
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn installing_ripgrep_runs_the_exact_installer_command() {
  let dir = tempfile::tempdir().expect("temp");
  let (program, args): (&str, &[&str]) = if cfg!(windows) {
    ("winget.exe", &["install", "--id", "BurntSushi.ripgrep.MSVC", "--exact"])
  } else {
    ("brew", &["install", "ripgrep"])
  };
  let installer = file(&dir.path().join(program), "installer");

  let plan = InstallPlan::find(Some(&path_of(&[dir.path()]))).expect("an installer");

  assert_eq!(plan.program(), installer);
  assert_eq!(plan.command().args(), args);
}
