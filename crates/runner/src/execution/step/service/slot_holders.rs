//! Who holds the computer's service slots (`super::slots`): beside each lock, `slot-<n>.holder` names the start that
//! took it last ([`Holding`] with what it starts and since when), as JSON. Only the lock's holder writes it, when it
//! takes the slot; it stays after the slot is freed, and names no one then, since only a held slot's record is read.
//! It lies beside the lock rather than in its file, as on Windows a held lock keeps everyone else from reading the
//! file it locks. What it says is untrusted input (any program of the user can write it; on macOS the lock is
//! advisory): it is read up to [`RECORD_BYTES`] and parsed strictly, and a record that does not parse names no one.
//!
//! A start that waits looks at the slots once ([`look`]): how many are free, and who holds the others, one holder per
//! start with the slots it holds. A free slot is found by taking its lock for a moment, which another start that
//! tries it meanwhile sees as held and tries again.

use std::{
  fs::{self, File, OpenOptions, TryLockError},
  io::{self, Read},
  path::{Path, PathBuf},
};

use orchester_core::{
  id::ProjectId,
  service::{ServiceSlotHolder, SlotStep, SlotUse},
  time::Timestamp,
};

/// The most of a holder record that is read: a project id, a task key, a step's name and a few service names.
const RECORD_BYTES: u64 = 8 * 1024;

/// Whose start takes service slots: its project, its task's key, its run's number and the step whose agent starts.
#[derive(Clone, Debug)]
pub(in crate::execution::step) struct Holding {
  pub(in crate::execution::step) project: ProjectId,
  pub(in crate::execution::step) task: String,
  pub(in crate::execution::step) run: u32,
  pub(in crate::execution::step) step: Option<SlotStep>,
}

impl Holding {
  /// The record of a start of `what`, taken now, as its slots' holder files keep it.
  pub(super) fn record(&self, what: &SlotUse) -> ServiceSlotHolder {
    ServiceSlotHolder {
      project: self.project,
      task: self.task.clone(),
      run: self.run,
      step: self.step.clone(),
      what: what.clone(),
      slots: 1,
      since: Timestamp::now(),
    }
  }
}

/// Names `record` as the holder of the computer's slots `slots` in `folder`; a record that cannot be written is
/// logged, and the slot is held all the same.
pub(super) fn claim(folder: &Path, slots: &[u32], record: &ServiceSlotHolder) {
  let text = match serde_json::to_string(record) {
    Ok(text) => text,
    Err(error) => {
      tracing::warn!(%error, "a service slot's holder cannot be written");
      return;
    }
  };
  for slot in slots {
    if let Err(error) = fs::write(holder_file(folder, *slot), &text) {
      tracing::warn!(%error, slot, "a service slot's holder cannot be written");
    }
  }
}

/// The lowest `upto` slots in `folder` as they are now: how many are free, and the holders of the others whose
/// records read, in slot order, the slots of one start counted together.
pub(super) fn look(folder: &Path, upto: u32) -> io::Result<(u32, Vec<ServiceSlotHolder>)> {
  fs::create_dir_all(folder)?;
  let mut free = 0;
  let mut holders: Vec<ServiceSlotHolder> = Vec::new();
  for slot in 0..upto {
    let file = OpenOptions::new()
      .create(true)
      .truncate(false)
      .read(true)
      .write(true)
      .open(lock_file(folder, slot))?;
    match file.try_lock() {
      // Free: closing the file frees it again.
      Ok(()) => free += 1,
      Err(TryLockError::WouldBlock) => {
        if let Some(record) = read(folder, slot) {
          match holders.iter_mut().find(|holder| same_start(holder, &record)) {
            Some(holder) => holder.slots += 1,
            None => holders.push(ServiceSlotHolder { slots: 1, ..record }),
          }
        }
      }
      Err(TryLockError::Error(error)) => return Err(error),
    }
  }
  Ok((free, holders))
}

/// Whether two records name the same start.
fn same_start(held: &ServiceSlotHolder, record: &ServiceSlotHolder) -> bool {
  (held.project, &held.task, held.run, &held.step, &held.what, held.since)
    == (record.project, &record.task, record.run, &record.step, &record.what, record.since)
}

/// The record of slot `slot` in `folder`; `None`, logged when it cannot be read, when it is missing or does not parse.
fn read(folder: &Path, slot: u32) -> Option<ServiceSlotHolder> {
  let mut text = String::new();
  let read = File::open(holder_file(folder, slot)).and_then(|file| file.take(RECORD_BYTES).read_to_string(&mut text));
  match read {
    Ok(_) => serde_json::from_str(&text).ok(),
    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
    Err(error) => {
      tracing::warn!(%error, slot, "a service slot's holder cannot be read");
      None
    }
  }
}

pub(super) fn lock_file(folder: &Path, slot: u32) -> PathBuf {
  folder.join(format!("slot-{slot}.lock"))
}

fn holder_file(folder: &Path, slot: u32) -> PathBuf {
  folder.join(format!("slot-{slot}.holder"))
}
