//! What becomes of each file of a run's profile folder in a step's seeded `target/`. Rustc replaces the artifacts it
//! rebuilds (a new file renamed over the old one, or the old one removed first), so those are linked. Some files are
//! written in place, which would change the run's file through the link:
//!
//! - cargo's fingerprints and dep-info (`.d`) files, lock files, and everything under `build/`: the build scripts'
//!   programs, their output and their `OUT_DIR`, where a script writes what it likes;
//! - what a linker writes: MSVC's `link.exe` overwrites the program and its `.pdb` it links (seen on Windows, ORC-544).
//!
//! Shared libraries (proc macros) with their debug info and import libraries are copied: a proc macro cargo had to
//! link again would be newer than every crate that uses it, and each of them would be rebuilt. Programs (tests,
//! binaries, examples) are left out: no crate builds on them, and cargo links one again when it is missing.

use std::{
  ffi::OsStr,
  path::{Component, Path},
};

const FINGERPRINTS: &str = ".fingerprint";
const BUILD_SCRIPTS: &str = "build";
const INCREMENTAL: &str = "incremental";
/// Cargo's lock of a profile folder, which cargo makes when it builds.
const CARGO_LOCK: &str = ".cargo-lock";
const DEBUG_SYMBOLS: &str = ".dSYM";

/// What becomes of a file of the run's profile folder in the step's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Seeding {
  Link,
  Copy,
  Leave,
}

/// Whether the folder `name` is left out with all it holds: a program's debug symbols on macOS, which go with it.
pub(super) fn leaves_folder(name: &OsStr) -> bool {
  name.to_string_lossy().ends_with(DEBUG_SYMBOLS)
}

/// What becomes of `path`, a file at `relative` inside a profile folder.
pub(super) fn seeding(path: &Path, relative: &Path) -> Seeding {
  let first = relative
    .components()
    .next()
    .map_or(OsStr::new(""), Component::as_os_str);
  if relative.file_name() == Some(OsStr::new(CARGO_LOCK)) {
    return Seeding::Leave;
  }
  if first == FINGERPRINTS || first == BUILD_SCRIPTS {
    return Seeding::Copy;
  }
  let Some(extension) = relative.extension().and_then(OsStr::to_str) else {
    // A program on macOS and Linux; incremental session files are rustc's own and replaced like its artifacts.
    return if first == INCREMENTAL {
      Seeding::Link
    } else {
      Seeding::Leave
    };
  };
  match extension {
    "d" | "lock" | "dll" | "so" | "dylib" | "lib" | "exp" => Seeding::Copy,
    "exe" => Seeding::Leave,
    "pdb" if path.with_extension("dll").is_file() => Seeding::Copy,
    "pdb" => Seeding::Leave,
    _ => Seeding::Link,
  }
}
