//! Moves a file or a folder to the Recycle Bin (Windows) or the Trash (macOS), where the person can still restore it;
//! never a permanent delete. A system shell deletes for good what its bin can't take (a drive without a bin, a bin
//! turned off, an item over the bin's size), so the volume's bin is checked first ([`Bin`]) and refused when it would
//! not keep the item; after the move the item must have left its place and reached the bin. Nothing here asks in a
//! dialog: a failure carries the system's own reason. Other platforms get an explicit error.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::{fmt, fs, io, path::Path};

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// Moves `path`, a file or a folder, to the Recycle Bin / Trash, refused before anything moves when its volume's bin
/// would not keep it.
pub fn move_to_trash(path: &Path) -> io::Result<()> {
  ensure_movable(path)?;
  bin_of(path)?.ensure_room(size_of(path)?)?;
  platform::recycle(path)
}

/// What the volume of a path does with what is moved to its Recycle Bin / Trash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bin {
  /// It has no bin, or its bin deletes what it is given for good: nothing may go there.
  Missing,
  /// It keeps what it is given, up to `capacity` bytes when the system says.
  Keeps { capacity: Option<u64> },
}

impl Bin {
  /// Refused unless this bin keeps an item of `size` bytes.
  pub fn ensure_room(self, size: u64) -> io::Result<()> {
    match self {
      Self::Missing => Err(TrashRefusal::NoBin.into()),
      Self::Keeps {
        capacity: Some(capacity),
      } if size > capacity => Err(TrashRefusal::TooLarge.into()),
      Self::Keeps { .. } => Ok(()),
    }
  }
}

/// Why the trash refused an item before or after moving it, for a caller to say in its own words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashRefusal {
  /// The volume has no Recycle Bin / Trash, or its bin deletes for good; nothing moved.
  NoBin,
  /// The item is larger than the bin keeps; nothing moved.
  TooLarge,
  /// The system took the item but its bin did not receive it: it was deleted.
  NotKept,
}

impl TrashRefusal {
  /// The refusal an error of [`move_to_trash`] carries, if it is one.
  pub fn of(error: &io::Error) -> Option<Self> {
    error.get_ref()?.downcast_ref::<Self>().copied()
  }
}

impl fmt::Display for TrashRefusal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::NoBin => "this drive has no Recycle Bin or Trash that keeps what it is given; nothing was moved",
      Self::TooLarge => "this is larger than the Recycle Bin or Trash keeps; nothing was moved",
      Self::NotKept => "the system deleted it instead of keeping it in the Recycle Bin or Trash",
    })
  }
}

impl std::error::Error for TrashRefusal {}

impl From<TrashRefusal> for io::Error {
  fn from(refusal: TrashRefusal) -> Self {
    let kind = match refusal {
      TrashRefusal::NoBin => io::ErrorKind::Unsupported,
      TrashRefusal::TooLarge => io::ErrorKind::InvalidInput,
      TrashRefusal::NotKept => io::ErrorKind::Other,
    };
    Self::new(kind, refusal)
  }
}

/// What the volume of `path` does with what goes to its bin.
pub fn bin_of(path: &Path) -> io::Result<Bin> {
  platform::bin_of(path)
}

/// Refused, with the system's reason, while another program holds the file `path` so that it can't move. A check
/// before the move, which itself never asks: a program that takes the file after it fails the move instead.
pub fn ensure_movable(path: &Path) -> io::Result<()> {
  platform::ensure_movable(path)
}

/// The bytes `path` holds, a folder's files counted, links not followed.
fn size_of(path: &Path) -> io::Result<u64> {
  let metadata = fs::symlink_metadata(path)?;
  if !metadata.is_dir() {
    return Ok(metadata.len());
  }
  let mut total = 0;
  for entry in fs::read_dir(path)? {
    total += size_of(&entry?.path())?;
  }
  Ok(total)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
  use std::{io, path::Path};

  use super::Bin;

  pub fn bin_of(_path: &Path) -> io::Result<Bin> {
    Ok(Bin::Missing)
  }

  pub fn ensure_movable(_path: &Path) -> io::Result<()> {
    Ok(())
  }

  pub fn recycle(path: &Path) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      format!("{} can't be moved to a trash on this platform yet", path.display()),
    ))
  }
}
