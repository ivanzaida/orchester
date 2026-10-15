//! What *Open in editor* opens: only text and code files, by extension, or a file without one that holds text; on
//! macOS in the default text editor and never a link or a file with an execute bit, on Windows with the default
//! program and never a text kind Windows would run; never a commit's file, which is not on disk.

use std::{fs, process};

use orchester_core::revision::Revision;
use orchester_exec::programs::Programs;
use orchester_workspace::files::{
  FilePath, OpenError, OpenFacts, OpenWith, Platform, Refusal, Version, open_target, open_with,
};

fn facts(name: &str) -> OpenFacts<'_> {
  OpenFacts {
    name,
    text: true,
    link: false,
    executable: false,
  }
}

#[test]
fn files_open_with_lets_only_text_and_code_open() {
  for platform in [Platform::Windows, Platform::MacOs] {
    for name in [
      "main.rs",
      "README.md",
      "Cargo.toml",
      "config.YAML",
      "data.json",
      "notes.txt",
      "Dockerfile",
      ".env",
    ] {
      assert!(
        open_with(platform, &facts(name)).is_ok(),
        "{name} opens on {platform:?}"
      );
    }
    for name in [
      "app.exe",
      "logo.png",
      "setup.msi",
      "link.lnk",
      "page.url",
      "archive.zip",
      "tool.jar",
      "x.hta",
    ] {
      assert_eq!(
        open_with(platform, &facts(name)),
        Err(Refusal::NotText),
        "{name} on {platform:?}"
      );
    }
    let binary = OpenFacts {
      text: false,
      ..facts("blob")
    };
    assert_eq!(
      open_with(platform, &binary),
      Err(Refusal::NotText),
      "a file without extension that is not text"
    );
    let lying = OpenFacts {
      text: false,
      ..facts("image.rs")
    };
    assert_eq!(
      open_with(platform, &lying),
      Err(Refusal::NotText),
      "a text extension on binary content"
    );
    let link = OpenFacts {
      link: true,
      ..facts("main.rs")
    };
    assert_eq!(open_with(platform, &link), Err(Refusal::Link));
  }
}

#[test]
fn files_open_with_opens_text_in_the_text_editor_on_macos_and_with_the_default_program_on_windows() {
  assert_eq!(open_with(Platform::MacOs, &facts("main.rs")), Ok(OpenWith::TextEditor));
  assert_eq!(open_with(Platform::MacOs, &facts("run.py")), Ok(OpenWith::TextEditor));
  let executable = OpenFacts {
    executable: true,
    ..facts("build.sh")
  };
  assert_eq!(open_with(Platform::MacOs, &executable), Err(Refusal::Executable));
  assert_eq!(
    open_with(Platform::Windows, &facts("main.rs")),
    Ok(OpenWith::DefaultProgram)
  );
  for name in [
    "run.py",
    "app.js",
    "build.sh",
    "make.BAT",
    "setup.ps1",
    "tool.pl",
    "gem.rb",
  ] {
    assert_eq!(
      open_with(Platform::Windows, &facts(name)),
      Err(Refusal::WouldRun),
      "{name}"
    );
  }
  assert_eq!(
    open_with(Platform::Windows, &executable),
    Err(Refusal::WouldRun),
    "Windows weighs the extension"
  );
}

#[test]
fn files_open_target_opens_a_text_file_on_disk_and_refuses_the_rest() {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let repo = dir.path();
  let git = |args: &[&str]| {
    let status = process::Command::new("git")
      .arg("-C")
      .arg(repo)
      .args(args)
      .output()
      .expect("git");
    assert!(status.status.success(), "git {args:?}");
    String::from_utf8_lossy(&status.stdout).trim().to_owned()
  };
  git(&["init", "--quiet", "--initial-branch=main"]);
  git(&["config", "user.name", "Orchester Test"]);
  git(&["config", "user.email", "test@orchester.invalid"]);
  fs::write(repo.join("main.rs"), "fn main() {}\n").expect("main.rs");
  fs::write(repo.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 1]).expect("logo");
  fs::write(repo.join("LICENSE"), "MIT\n").expect("license");
  git(&["add", "."]);
  git(&["commit", "--quiet", "-m", "first"]);
  let head = Revision::new(&git(&["rev-parse", "HEAD"])).expect("a commit");
  let programs = Programs::processes();
  let open = |name: &str, at: &Version| {
    let path = FilePath::new(name).expect("a path");
    open_target(programs.runner(), repo, at, &path)
  };
  let (opened, with) = open("main.rs", &Version::Checkout).expect("main.rs opens");
  assert!(opened.is_absolute() && opened.ends_with("main.rs"));
  let expected = if cfg!(target_os = "macos") {
    OpenWith::TextEditor
  } else {
    OpenWith::DefaultProgram
  };
  assert_eq!(with, expected);
  assert!(
    open("LICENSE", &Version::Checkout).is_ok(),
    "a file without extension that holds text"
  );
  assert_eq!(
    open("logo.png", &Version::Checkout),
    Err(OpenError::Refused(
      FilePath::new("logo.png").expect("a path"),
      Refusal::NotText
    ))
  );
  assert_eq!(open("main.rs", &Version::Revision(head)), Err(OpenError::NotOnDisk));
  #[cfg(unix)]
  links_and_executables_are_refused(repo, &open);
}

#[cfg(unix)]
fn links_and_executables_are_refused(
  repo: &std::path::Path,
  open: &dyn Fn(&str, &Version) -> Result<(std::path::PathBuf, OpenWith), OpenError>,
) {
  use std::os::unix::fs::{PermissionsExt, symlink};
  fs::write(repo.join("build.sh"), "echo hi\n").expect("a script");
  fs::set_permissions(repo.join("build.sh"), fs::Permissions::from_mode(0o755)).expect("executable");
  symlink(repo.join("main.rs"), repo.join("alias.rs")).expect("a link");
  if cfg!(target_os = "macos") {
    assert!(matches!(
      open("build.sh", &Version::Checkout),
      Err(OpenError::Refused(_, Refusal::Executable))
    ));
  }
  assert!(matches!(
    open("alias.rs", &Version::Checkout),
    Err(OpenError::Refused(_, Refusal::Link))
  ));
}
