//! The QA slot: one QA app runs on the computer at a time ([`QA_SLOTS`]), whichever runner and project starts it. A
//! slot is an OS lock on a file of the user's data folder, which every runner takes the same way; the OS releases it
//! when its holder closes the file or ends, so a runner that died never keeps a slot.

use std::{
  fs::{self, File, OpenOptions, TryLockError},
  io,
  path::{Path, PathBuf},
};

/// How many QA apps run on the computer at once.
const QA_SLOTS: usize = 1;

/// Where the computer's QA slots are.
#[derive(Clone, Debug)]
pub struct QaSlots {
  /// `None` on a platform without a user data folder: no QA app can start there yet.
  folder: Option<PathBuf>,
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

  /// A free slot, or `None` while every slot is held.
  pub(super) fn try_take(&self) -> Result<Option<Lease>, SlotError> {
    let folder = self.folder.as_deref().ok_or(SlotError::Unsupported)?;
    fs::create_dir_all(folder).map_err(SlotError::Io)?;
    for slot in 0..QA_SLOTS {
      let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(folder.join(format!("slot-{slot}.lock")))
        .map_err(SlotError::Io)?;
      match file.try_lock() {
        Ok(()) => return Ok(Some(Lease { _file: file })),
        Err(TryLockError::WouldBlock) => {}
        Err(TryLockError::Error(error)) => return Err(SlotError::Io(error)),
      }
    }
    Ok(None)
  }
}

/// A held slot; dropping it closes the file and frees the slot.
pub(super) struct Lease {
  _file: File,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum SlotError {
  #[error("QA apps are not supported on this platform yet: it has no user data folder for the QA slot")]
  Unsupported,
  #[error("the QA slot cannot be taken: {0}")]
  Io(io::Error),
}
