//! Downloading an offered installer into the updates folder and checking it before anything keeps it: the bytes go to
//! `<name>.part`, resumed with `Range` from where an interrupted download stopped; once complete, its size and SHA-256
//! must be the release's and its signature must verify with a release key built into the app. Only then does it
//! become `<name>`; a file that fails is deleted. Every other file in the folder is an older download and goes.

use std::{
  fs::{self, File, OpenOptions},
  io::{self, Read, Write},
  path::{Path, PathBuf},
};

use orchester_api::{
  client::{ClientError, CloudClient, ReleaseDownload},
  releases::{AcceptedReleaseKeys, FileDigest, OfferedFile, OfferedRelease, SignedFile},
};
use sha2::{Digest, Sha256};

use super::state::UpdateFailure;
use crate::background;

/// What a download came to.
#[derive(Debug)]
pub(super) enum Fetched {
  /// The installer is complete and verified, at this path.
  Verified(PathBuf),
  /// The file failed a check and was deleted, or could not be saved.
  Failed(UpdateFailure),
  /// The cloud could not be reached or broke off; what arrived is kept to resume from.
  Interrupted(ClientError),
}

/// One offered file to download into `folder`, checked with `keys`.
#[derive(Clone)]
pub(super) struct Download {
  pub folder: PathBuf,
  pub release: OfferedRelease,
  pub file: OfferedFile,
  pub keys: AcceptedReleaseKeys,
}

impl Download {
  fn done(&self) -> PathBuf {
    self.folder.join(self.file.name.as_str())
  }

  fn part(&self) -> PathBuf {
    self.folder.join(format!("{}.part", self.file.name))
  }

  /// Downloads the file with `client`, telling `progress` the bytes there are so far, and checks it.
  pub async fn fetch(&self, client: &CloudClient, progress: &(dyn Fn(u64) + Sync)) -> Fetched {
    let from = match self.off_thread(Self::prepare).await {
      Some(Ok(Prepared::Verified(path))) => return Fetched::Verified(path),
      Some(Ok(Prepared::From(from))) => from,
      Some(Err(error)) => return not_saved(&error),
      None => return Fetched::Failed(UpdateFailure::NotSaved),
    };
    progress(from);
    if from < self.file.size {
      let answer = client.download(&self.release.version, &self.file.name, from).await;
      match answer {
        Ok(download) => {
          if let Some(fetched) = self.receive(download, from, progress).await {
            return fetched;
          }
        }
        Err(error) => {
          // The part is longer than the file the cloud has: it starts over at the next check.
          if matches!(error, ClientError::Cloud { status: 416, .. }) {
            self.discard();
          }
          return Fetched::Interrupted(error);
        }
      }
    }
    self
      .off_thread(Self::finish)
      .await
      .unwrap_or(Fetched::Failed(UpdateFailure::NotSaved))
  }

  /// Runs `work` on a clone of this download on the blocking pool: the file checks read the whole file. `None` when
  /// the work did not finish, which `background::run` logs.
  async fn off_thread<T: Send + 'static>(&self, work: fn(&Self) -> T) -> Option<T> {
    let download = self.clone();
    background::run(move || work(&download)).await.ok()
  }

  /// Removes the older downloads and finds where this one stands: verified already, or the byte to go on from.
  fn prepare(&self) -> io::Result<Prepared> {
    fs::create_dir_all(&self.folder)?;
    let (done, part) = (self.done(), self.part());
    for entry in fs::read_dir(&self.folder)? {
      let path = entry?.path();
      if path != done && path != part {
        remove(&path)?;
      }
    }
    if done.is_file() {
      match self.verify(&done) {
        Ok(()) => return Ok(Prepared::Verified(done)),
        Err(failure) => {
          tracing::warn!(?failure, file = %done.display(), "a downloaded update no longer verifies; downloading it again");
          remove(&done)?;
        }
      }
    }
    let have = match fs::metadata(&part) {
      Ok(metadata) => metadata.len(),
      Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
      Err(error) => return Err(error),
    };
    if have > self.file.size {
      remove(&part)?;
      return Ok(Prepared::From(0));
    }
    Ok(Prepared::From(have))
  }

  /// Writes the answer's bytes to the part file; `None` once all of them are there, else what the download came to.
  async fn receive(
    &self,
    mut download: ReleaseDownload,
    from: u64,
    progress: &(dyn Fn(u64) + Sync),
  ) -> Option<Fetched> {
    if download.total() != self.file.size {
      self.discard();
      return Some(Fetched::Failed(UpdateFailure::WrongSize));
    }
    // A cloud that sent the whole file instead of the range starts it over; another start is not the asked one.
    let start = download.start();
    if start != from && start != 0 {
      self.discard();
      return Some(Fetched::Interrupted(ClientError::NotOrchester { status: 206 }));
    }
    let mut file = match self.open_part(start == 0) {
      Ok(file) => file,
      Err(error) => return Some(not_saved(&error)),
    };
    let mut received = start;
    loop {
      let chunk = match download.chunk().await {
        Ok(Some(chunk)) => chunk,
        Ok(None) => return None,
        Err(error) => return Some(Fetched::Interrupted(error)),
      };
      received += chunk.len() as u64;
      if received > self.file.size {
        self.discard();
        return Some(Fetched::Failed(UpdateFailure::WrongSize));
      }
      // A chunk is a few kilobytes: writing it here holds the runtime's thread no longer than reading it did.
      if let Err(error) = file.write_all(&chunk) {
        return Some(not_saved(&error));
      }
      progress(received);
    }
  }

  fn open_part(&self, from_start: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true);
    if from_start {
      options.write(true).truncate(true);
    } else {
      options.append(true);
    }
    options.open(self.part())
  }

  /// Checks the complete part file and keeps it as the installer, or deletes it.
  fn finish(&self) -> Fetched {
    let part = self.part();
    if let Err(failure) = self.verify(&part) {
      tracing::warn!(?failure, version = %self.release.version, "a downloaded update failed its check and is deleted");
      self.discard();
      return Fetched::Failed(failure);
    }
    let done = self.done();
    match fs::rename(&part, &done) {
      Ok(()) => Fetched::Verified(done),
      Err(error) => {
        self.discard();
        not_saved(&error)
      }
    }
  }

  /// Whether the file at `path` is the offered one: its size, its SHA-256 and its signature.
  fn verify(&self, path: &Path) -> Result<(), UpdateFailure> {
    let size = fs::metadata(path).map_err(|error| logged(&error))?.len();
    if size != self.file.size {
      return Err(UpdateFailure::WrongSize);
    }
    let sha256 = digest(path).map_err(|error| logged(&error))?;
    if sha256 != self.file.sha256 {
      return Err(UpdateFailure::WrongDigest);
    }
    let signed = SignedFile {
      version: self.release.version.clone(),
      platform: self.file.platform,
      arch: self.file.arch,
      name: self.file.name.clone(),
      size,
      sha256,
    };
    self
      .keys
      .verify(&signed, self.file.key_id, &self.file.signature)
      .map_err(UpdateFailure::from)
  }

  /// Deletes the part file: nothing unverified is kept.
  fn discard(&self) {
    if let Err(error) = remove(&self.part()) {
      tracing::error!(%error, "cannot delete an unverified update download");
    }
  }
}

enum Prepared {
  Verified(PathBuf),
  From(u64),
}

/// Removes every download in `folder`, once the cloud offers nothing newer. Blocking.
pub(super) fn clear(folder: &Path) -> io::Result<()> {
  match fs::read_dir(folder) {
    Ok(entries) => entries.into_iter().try_for_each(|entry| remove(&entry?.path())),
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
    Err(error) => Err(error),
  }
}

/// Removes the file or folder at `path`; one already gone is fine.
fn remove(path: &Path) -> io::Result<()> {
  let removed = if path.is_dir() {
    fs::remove_dir_all(path)
  } else {
    fs::remove_file(path)
  };
  match removed {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}

fn digest(path: &Path) -> io::Result<FileDigest> {
  let mut file = File::open(path)?;
  let mut hash = Sha256::new();
  let mut buffer = vec![0_u8; 64 * 1024];
  loop {
    let read = file.read(&mut buffer)?;
    if read == 0 {
      return Ok(FileDigest::from_bytes(hash.finalize().into()));
    }
    hash.update(&buffer[..read]);
  }
}

fn logged(error: &io::Error) -> UpdateFailure {
  tracing::error!(%error, "cannot read or write the updates folder");
  UpdateFailure::NotSaved
}

fn not_saved(error: &io::Error) -> Fetched {
  Fetched::Failed(logged(error))
}
