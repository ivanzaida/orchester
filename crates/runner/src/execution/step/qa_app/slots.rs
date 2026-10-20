//! The QA slots: how many QA apps run on the computer at once, whichever runner and project starts them. A slot is an
//! OS lock on a file of the user's data folder, `slot-<n>.lock`, which every runner takes the same way; the OS releases
//! it when its holder closes the file or ends, so a runner that died never keeps a slot.
//!
//! How many slots a start may use is its project's *QA apps at once* (`orchester_core::limits::QaSlots`), read at each
//! try: a runner whose project allows N tries slots 0 to N−1 in order and takes the first free one. Every project's
//! runner shares the same files, so projects with different limits share the low slots: a project that allows 2 waits
//! while slots 0 and 1 are held, also when another project that allows 4 holds them, and that one may still take slots
//! 2 and 3. Together they never have more apps open than the larger limit. A slot's file is made the first time a start
//! tries it, so the files of slots no project's limit reaches are never made.
//!
//! Beside each lock, `slot-<n>.holder` names the slot's latest holder ([`Holder`]): its QA folder, when its run was
//! created, its task's key and its run's number. Only the lock's holder writes it ([`Lease::claim`]), and it stays
//! after the slot is freed: the next holder of the same slot reads it to stop what a holder that died left running
//! there (`super::leftover`). While the slot is held, others read it too: a start that waits names who holds the slots,
//! and the cleanup leaves alone a folder a held slot names ([`Lease::held_elsewhere`]). It lies beside the lock rather
//! than in its file, as on Windows a held lock keeps everyone else from reading the file it locks. What it says is
//! untrusted input (any program of the user can write it; on macOS the lock is advisory): it is read up to
//! [`HOLDER_BYTES`] and parsed strictly, and a record that does not parse names no one.

use std::{
  fs::{self, File, OpenOptions, TryLockError},
  io::{self, Read},
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use orchester_core::{limits::QaSlots as Allowed, project::TaskKey, run::RunNumber, time::Timestamp};

/// The most of a holder record that is read: a folder path and three short lines fit well within it.
const HOLDER_BYTES: u64 = 8 * 1024;

/// Where the computer's QA slots are.
#[derive(Clone, Debug)]
pub struct QaSlots {
  /// `None` on a platform without a user data folder: no QA app can start there yet.
  folder: Option<PathBuf>,
}

/// What a try for a slot found.
pub(super) enum Take {
  Taken(Lease),
  /// Every slot the start may use is held, by these holders in slot order; `None` when a held slot names no holder
  /// that reads.
  AllHeld(Option<Vec<Holder>>),
}

impl QaSlots {
  /// The computer's slots, in the user's data folder.
  pub fn machine() -> Self {
    Self {
      folder: super::super::run_app::user_data().map(|data| data.join("orchester-qa-slots")),
    }
  }

  /// Slots in `folder`, shared only by the runners given the same folder.
  pub fn in_folder(folder: &Path) -> Self {
    Self {
      folder: Some(folder.to_path_buf()),
    }
  }

  /// The first free slot of the `allowed` lowest, or who holds them while every one is held.
  pub(super) fn try_take(&self, allowed: Allowed) -> Result<Take, SlotError> {
    let folder = self.folder.as_deref().ok_or(SlotError::Unsupported)?;
    fs::create_dir_all(folder).map_err(SlotError::Io)?;
    let mut holders = Vec::new();
    for slot in 0..allowed.get().get() {
      let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_file(folder, slot))
        .map_err(SlotError::Io)?;
      match file.try_lock() {
        Ok(()) => {
          return Ok(Take::Taken(Lease {
            _lock: file,
            folder: folder.to_path_buf(),
            slot,
          }));
        }
        Err(TryLockError::WouldBlock) => holders.push(Holder::of(folder, slot)),
        Err(TryLockError::Error(error)) => return Err(SlotError::Io(error)),
      }
    }
    Ok(Take::AllHeld(holders.into_iter().collect()))
  }
}

/// A held slot; dropping it closes the lock's file and frees the slot.
pub(super) struct Lease {
  /// Held open for as long as the slot is this lease's.
  _lock: File,
  folder: PathBuf,
  slot: u32,
}

impl Lease {
  /// Names `holder` as the slot's, and answers the previous holder its record names, if it names one that reads.
  pub(super) fn claim(&mut self, holder: &Holder) -> io::Result<Option<Holder>> {
    let previous = Holder::of(&self.folder, self.slot);
    fs::write(holder_file(&self.folder, self.slot), holder.record())?;
    Ok(previous)
  }

  /// The holders of the other slots held now, whatever project's runner holds them; `None` when one of them names no
  /// holder that reads, or a slot cannot be looked at. A free slot is found by taking its lock for a moment, which
  /// another start that tries it meanwhile sees as held.
  pub(super) fn held_elsewhere(&self) -> Option<Vec<Holder>> {
    let mut holders = Vec::new();
    for slot in (0..Allowed::MAX).filter(|slot| *slot != self.slot) {
      let file = match File::open(lock_file(&self.folder, slot)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
        Err(error) => {
          tracing::warn!(%error, slot, "a QA slot cannot be looked at");
          return None;
        }
      };
      match file.try_lock() {
        // Free: closing the file frees it again.
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => holders.push(Holder::of(&self.folder, slot)?),
        Err(TryLockError::Error(error)) => {
          tracing::warn!(%error, slot, "a QA slot cannot be looked at");
          return None;
        }
      }
    }
    Some(holders)
  }
}

/// Who holds a slot: the QA folder of its app, when its run was created, its task's key and its run's number, written
/// as four lines: the creation in milliseconds since the Unix epoch, the key, the number, then the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Holder {
  pub(super) folder: PathBuf,
  pub(super) since: Timestamp,
  pub(super) task: TaskKey,
  pub(super) run: RunNumber,
}

impl Holder {
  /// The holder slot `slot`'s record in `folder` names; `None`, logged, when it cannot be read, and when it is missing
  /// or does not parse.
  fn of(folder: &Path, slot: u32) -> Option<Self> {
    let mut text = String::new();
    let read = File::open(holder_file(folder, slot)).and_then(|file| file.take(HOLDER_BYTES).read_to_string(&mut text));
    match read {
      Ok(_) => Self::read(&text),
      Err(error) if error.kind() == io::ErrorKind::NotFound => None,
      Err(error) => {
        tracing::warn!(%error, slot, "a QA slot's holder cannot be read");
        None
      }
    }
  }

  /// The holder `text` names; `None` for text that is not four such lines with an absolute folder.
  fn read(text: &str) -> Option<Self> {
    let mut lines = text.trim().splitn(4, '\n');
    let since = lines.next()?.trim().parse::<i64>().ok().filter(|millis| *millis >= 0)?;
    let task = lines.next()?.trim().parse::<TaskKey>().ok()?;
    let run = lines.next()?.trim().parse::<NonZeroU32>().ok()?;
    let folder = PathBuf::from(lines.next()?.trim());
    folder.is_absolute().then(|| Self {
      folder,
      since: Timestamp::from_unix_millis(since),
      task,
      run: RunNumber::new(run),
    })
  }

  fn record(&self) -> String {
    format!(
      "{}\n{}\n{}\n{}",
      self.since.unix_millis(),
      self.task,
      self.run.get(),
      self.folder.to_string_lossy()
    )
  }
}

fn lock_file(folder: &Path, slot: u32) -> PathBuf {
  folder.join(format!("slot-{slot}.lock"))
}

fn holder_file(folder: &Path, slot: u32) -> PathBuf {
  folder.join(format!("slot-{slot}.holder"))
}

#[derive(Debug, thiserror::Error)]
pub(super) enum SlotError {
  #[error("QA apps are not supported on this platform yet: it has no user data folder for the QA slots")]
  Unsupported,
  #[error("a QA slot cannot be taken: {0}")]
  Io(io::Error),
}
