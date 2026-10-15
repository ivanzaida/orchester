//! The QA slot: one QA app runs on the computer at a time ([`QA_SLOTS`]), whichever runner and project starts it. A
//! slot is an OS lock on a file of the user's data folder, which every runner takes the same way; the OS releases it
//! when its holder closes the file or ends, so a runner that died never keeps a slot. The file names the QA folder of
//! its latest holder and when that holder's run was created ([`Lease::claim`]), so the next holder can stop an app a
//! holder that died left running there; what it reads there is checked as untrusted input (`super::leftover`).

use std::{
  fs::{self, File, OpenOptions, TryLockError},
  io::{self, Read, Seek, SeekFrom, Write},
  path::{Path, PathBuf},
};

use orchester_core::time::Timestamp;

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
        .read(true)
        .write(true)
        .open(folder.join(format!("slot-{slot}.lock")))
        .map_err(SlotError::Io)?;
      match file.try_lock() {
        Ok(()) => return Ok(Some(Lease { file })),
        Err(TryLockError::WouldBlock) => {}
        Err(TryLockError::Error(error)) => return Err(SlotError::Io(error)),
      }
    }
    Ok(None)
  }
}

/// A held slot; dropping it closes the file and frees the slot.
pub(super) struct Lease {
  file: File,
}

/// Who holds a slot: the QA folder of its app and when its run was created, written as two lines, the creation in
/// milliseconds since the Unix epoch first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Holder {
  pub(super) folder: PathBuf,
  pub(super) since: Timestamp,
}

impl Holder {
  /// The holder `text` names; `None` for text that is not two such lines with an absolute folder.
  fn read(text: &str) -> Option<Self> {
    let (since, folder) = text.trim().split_once('\n')?;
    let since = since.trim().parse::<i64>().ok().filter(|millis| *millis >= 0)?;
    let folder = PathBuf::from(folder.trim());
    folder.is_absolute().then(|| Self {
      folder,
      since: Timestamp::from_unix_millis(since),
    })
  }
}

impl Lease {
  /// Names `holder` as the slot's, and answers the previous holder the file names, if it names one.
  pub(super) fn claim(&mut self, holder: &Holder) -> io::Result<Option<Holder>> {
    let mut previous = String::new();
    self.file.seek(SeekFrom::Start(0))?;
    self.file.read_to_string(&mut previous)?;
    self.file.set_len(0)?;
    self.file.seek(SeekFrom::Start(0))?;
    write!(
      self.file,
      "{}\n{}",
      holder.since.unix_millis(),
      holder.folder.to_string_lossy()
    )?;
    Ok(Holder::read(&previous))
  }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum SlotError {
  #[error("QA apps are not supported on this platform yet: it has no user data folder for the QA slot")]
  Unsupported,
  #[error("the QA slot cannot be taken: {0}")]
  Io(io::Error),
}
