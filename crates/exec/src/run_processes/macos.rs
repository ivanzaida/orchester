#![allow(unsafe_code)]
//! libproc PID enumeration has no safe wrapper. Executable lookup and signals are bound to an audit token, so
//! neither PID reuse nor exec between validation and signalling can redirect a stop to another executable (before
//! macOS 15 the signal re-checks the token immediately before it; see `identity`).

mod identity;

use std::{io, mem};

use identity::{AuditToken, read};

use super::{Entry, Info};

pub(super) fn snapshot() -> io::Result<Vec<Entry>> {
  let mut capacity = 1024usize;
  loop {
    let mut pids = vec![0i32; capacity];
    // SAFETY: buffer is writable for its stated byte length; libproc returns the count, not bytes.
    let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), (capacity * mem::size_of::<i32>()) as i32) };
    if count < 0 {
      return Err(io::Error::last_os_error());
    }
    if count as usize >= capacity {
      capacity *= 2;
      if capacity > 1_048_576 {
        return Err(io::Error::other("process list exceeded its bound"));
      }
      continue;
    }
    let mut result = Vec::new();
    for pid in pids.into_iter().take(count as usize).filter(|pid| *pid > 0) {
      // Exited, other users' or otherwise uninspectable processes can never pass the executable rule, so one of
      // them never fails the whole list.
      if let Ok(Some(info)) = read(pid as u32) {
        result.push(Entry {
          pid: info.bsd.pbi_pid,
          parent: info.bsd.pbi_ppid,
          unique_id: info.unique.unique_id,
          parent_unique_id: info.unique.parent_unique_id,
        });
      }
    }
    return Ok(result);
  }
}

pub(super) struct Process {
  pub(super) info: Info,
  unique_id: u64,
  token: AuditToken,
}

impl Process {
  pub(super) fn open(entry: Entry, _stopping: bool) -> io::Result<Option<Self>> {
    let Some(current) = read(entry.pid)? else {
      return Ok(None);
    };
    if current.exited() || (entry.unique_id != 0 && entry.unique_id != current.unique.unique_id) {
      return Ok(None);
    }
    let token = current.token();
    let path = match token.image() {
      Ok(Some(path)) => path,
      result => {
        // Distinguish exit from exec: only disappearance is "already gone".
        let Some(after) = read(entry.pid)? else {
          return Ok(None);
        };
        if after.exited() || after.unique.unique_id != current.unique.unique_id {
          return Ok(None);
        }
        return Err(result.err().unwrap_or_else(|| {
          io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cannot verify executable identity; refused",
          )
        }));
      }
    };
    let started = current
      .bsd
      .pbi_start_tvsec
      .checked_mul(1_000_000)
      .and_then(|micros| micros.checked_add(current.bsd.pbi_start_tvusec))
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process start time out of range"))?;
    Ok(Some(Self {
      info: Info {
        pid: entry.pid,
        path,
        started,
      },
      unique_id: current.unique.unique_id,
      token,
    }))
  }

  pub(super) fn is_parent_of(&self, entry: Entry) -> bool {
    // Birth ancestry survives reparenting to launchd when a close handler exits.
    entry.parent_unique_id == self.unique_id
  }

  pub(super) fn is_process(&self, entry: Entry) -> bool {
    entry.unique_id == self.unique_id
  }

  pub(super) fn alive(&self) -> io::Result<bool> {
    let Some(current) = read(self.info.pid)? else {
      return Ok(false);
    };
    if current.exited() || current.unique.unique_id != self.unique_id {
      return Ok(false);
    }
    // Exec changes pidversion even for the same executable; do not refresh the grant to signal it.
    if current.token() == self.token {
      Ok(true)
    } else {
      Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "process changed its executable identity; refused",
      ))
    }
  }

  pub(super) fn close(&self) -> io::Result<()> {
    self.signal(libc::SIGTERM)
  }

  pub(super) fn end(&self) -> io::Result<()> {
    self.signal(libc::SIGKILL)
  }

  fn signal(&self, signal: i32) -> io::Result<()> {
    match self.token.signal(signal) {
      Ok(()) => Ok(()),
      Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {
        if self.alive()? {
          Err(error)
        } else {
          Ok(())
        }
      }
      Err(error) => Err(error),
    }
  }
}
