//! Pins the two programs a UI verifier launches outside a shared Cargo target directory.

use std::{
  fs::{self, File, OpenOptions},
  io::{self, Read, Seek, SeekFrom, Write},
  path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifierPlatform {
  Windows,
  Macos,
}

impl VerifierPlatform {
  pub fn host() -> Result<Self, PinError> {
    match std::env::consts::OS {
      "windows" => Ok(Self::Windows),
      "macos" => Ok(Self::Macos),
      other => Err(PinError::UnsupportedPlatform(other.to_owned())),
    }
  }

  pub const fn program_names(self) -> (&'static str, &'static str) {
    match self {
      Self::Windows => ("orchester.exe", "orchester-runner.exe"),
      Self::Macos => ("orchester", "orchester-runner"),
    }
  }
}

#[derive(Debug, Error)]
pub enum PinError {
  #[error("usage: pin-verifier-app <built-program-directory> <run-owned-directory>")]
  Usage,
  #[error("UI verifier app pinning is unsupported on {0}")]
  UnsupportedPlatform(String),
  #[error("cannot access {path}: {source}")]
  Io {
    path: PathBuf,
    #[source]
    source: io::Error,
  },
  #[error("{0} changed while the UI verifier app was being copied; rebuild and pin again")]
  SourceChanged(PathBuf),
  #[error("cannot write the pinned app path: {0}")]
  Output(#[source] io::Error),
}

#[derive(Debug)]
pub struct PinnedVerifierApp {
  desktop: PathBuf,
  runner: PathBuf,
}

impl PinnedVerifierApp {
  pub fn desktop(&self) -> &Path {
    &self.desktop
  }

  pub fn runner(&self) -> &Path {
    &self.runner
  }
}

/// Copies the current desktop and runner into a unique directory under `run_dir`.
/// The returned path is safe from subsequent rebuilds of `source_dir`.
pub fn pin_verifier_app(
  source_dir: &Path,
  run_dir: &Path,
  platform: VerifierPlatform,
) -> Result<PinnedVerifierApp, PinError> {
  let source_dir = canonicalize(source_dir)?;
  let run_dir = canonicalize(run_dir)?;
  let staging = tempfile::Builder::new()
    .prefix("verifier-app-")
    .tempdir_in(&run_dir)
    .map_err(|source| PinError::Io { path: run_dir, source })?;
  let (desktop_name, runner_name) = platform.program_names();
  for name in [desktop_name, runner_name] {
    pin_program(&source_dir.join(name), &staging.path().join(name))?;
  }
  for name in [desktop_name, runner_name] {
    let source = source_dir.join(name);
    let pinned = staging.path().join(name);
    if digest_path(&source)? != digest_path(&pinned)? {
      return Err(PinError::SourceChanged(source));
    }
  }
  let destination = staging.keep();
  Ok(PinnedVerifierApp {
    desktop: destination.join(desktop_name),
    runner: destination.join(runner_name),
  })
}

fn pin_program(source_path: &Path, destination_path: &Path) -> Result<(), PinError> {
  let mut source = open(source_path)?;
  let before = digest(&mut source).map_err(|source| PinError::Io {
    path: source_path.into(),
    source,
  })?;
  source.seek(SeekFrom::Start(0)).map_err(|source| PinError::Io {
    path: source_path.into(),
    source,
  })?;
  let mut destination = OpenOptions::new()
    .read(true)
    .write(true)
    .create_new(true)
    .open(destination_path)
    .map_err(|source| PinError::Io {
      path: destination_path.into(),
      source,
    })?;
  io::copy(&mut source, &mut destination).map_err(|source| PinError::Io {
    path: destination_path.into(),
    source,
  })?;
  destination.flush().map_err(|source| PinError::Io {
    path: destination_path.into(),
    source,
  })?;
  let source_after = digest(&mut source).map_err(|source| PinError::Io {
    path: source_path.into(),
    source,
  })?;
  let destination_digest = digest(&mut destination).map_err(|source| PinError::Io {
    path: destination_path.into(),
    source,
  })?;
  if before != source_after || before != destination_digest {
    return Err(PinError::SourceChanged(source_path.into()));
  }
  let permissions = source
    .metadata()
    .map_err(|source| PinError::Io {
      path: source_path.into(),
      source,
    })?
    .permissions();
  fs::set_permissions(destination_path, permissions).map_err(|source| PinError::Io {
    path: destination_path.into(),
    source,
  })?;
  Ok(())
}

fn canonicalize(path: &Path) -> Result<PathBuf, PinError> {
  path.canonicalize().map_err(|source| PinError::Io {
    path: path.into(),
    source,
  })
}

fn open(path: &Path) -> Result<File, PinError> {
  File::open(path).map_err(|source| PinError::Io {
    path: path.into(),
    source,
  })
}

fn digest_path(path: &Path) -> Result<[u8; 32], PinError> {
  let mut file = open(path)?;
  digest(&mut file).map_err(|source| PinError::Io {
    path: path.into(),
    source,
  })
}

fn digest(file: &mut File) -> io::Result<[u8; 32]> {
  file.seek(SeekFrom::Start(0))?;
  let mut hasher = Sha256::new();
  let mut buffer = [0_u8; 64 * 1024];
  loop {
    let count = file.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    hasher.update(&buffer[..count]);
  }
  Ok(hasher.finalize().into())
}
