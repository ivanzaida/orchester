#![allow(unsafe_code)]
//! libproc has no safe wrapper in std or rustix. Its typed records supply executable paths and microsecond
//! creation times; each signal rechecks both. Signals use rustix's safe kill wrapper.
//! ABI: https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info.h

use std::{ffi::OsString, io, mem, os::unix::ffi::OsStringExt, path::PathBuf};

use rustix::process::{Pid, Signal, kill_process};

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
      match bsd_info(pid as u32) {
        Ok(Some(info)) => result.push(Entry {
          pid: info.pbi_pid,
          parent: info.pbi_ppid,
        }),
        Ok(None) => {}
        // Other users' protected processes are not inspectable; they can never pass the executable rule.
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {}
        Err(error) => return Err(error),
      }
    }
    return Ok(result);
  }
}

pub(super) struct Process {
  pub(super) info: Info,
}

impl Process {
  pub(super) fn open(entry: Entry, _stopping: bool) -> io::Result<Option<Self>> {
    let Some(bsd) = bsd_info(entry.pid)? else {
      return Ok(None);
    };
    if bsd.pbi_status == 5 {
      return Ok(None);
    }
    let path = image(entry.pid)?;
    // A PID reused between the path and BSD queries must not combine identities.
    let Some(after) = bsd_info(entry.pid)? else {
      return Ok(None);
    };
    if (bsd.pbi_start_tvsec, bsd.pbi_start_tvusec) != (after.pbi_start_tvsec, after.pbi_start_tvusec) {
      return Ok(None);
    }
    Ok(Some(Self {
      info: Info {
        pid: entry.pid,
        path,
        started: bsd.pbi_start_tvsec * 1_000_000 + bsd.pbi_start_tvusec,
      },
    }))
  }

  pub(super) fn alive(&self) -> io::Result<bool> {
    let entry = Entry {
      pid: self.info.pid,
      parent: 0,
    };
    let Some(current) = Self::open(entry, false)? else {
      return Ok(false);
    };
    if current.info.started != self.info.started {
      return Ok(false);
    }
    if current.info.path != self.info.path {
      return Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "process changed its executable; refused",
      ));
    }
    Ok(true)
  }

  pub(super) fn close(&self) -> io::Result<()> {
    self.signal(Signal::TERM)
  }

  pub(super) fn end(&self) -> io::Result<()> {
    self.signal(Signal::KILL)
  }

  fn signal(&self, signal: Signal) -> io::Result<()> {
    if !self.alive()? {
      return Ok(());
    }
    let pid = i32::try_from(self.info.pid)
      .ok()
      .and_then(Pid::from_raw)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid process id"))?;
    match kill_process(pid, signal) {
      Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
      Err(error) => Err(error.into()),
    }
  }
}

fn bsd_info(pid: u32) -> io::Result<Option<libc::proc_bsdinfo>> {
  let pid = i32::try_from(pid).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid process id"))?;
  // SAFETY: proc_bsdinfo is a C record with integer fields and byte arrays, all valid zeroed.
  let mut info: libc::proc_bsdinfo = unsafe { mem::zeroed() };
  let size = mem::size_of::<libc::proc_bsdinfo>() as i32;
  // SAFETY: the record has exactly the documented PROC_PIDTBSDINFO buffer size and alignment.
  let read = unsafe {
    libc::proc_pidinfo(
      pid,
      libc::PROC_PIDTBSDINFO,
      0,
      (&mut info as *mut libc::proc_bsdinfo).cast(),
      size,
    )
  };
  if read == size {
    return Ok(Some(info));
  }
  let error = io::Error::last_os_error();
  if error.raw_os_error() == Some(libc::ESRCH) {
    Ok(None)
  } else {
    Err(error)
  }
}

fn image(pid: u32) -> io::Result<PathBuf> {
  let mut bytes = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
  // SAFETY: BSD lookup checked that PID fits i32; the buffer has the required maximum byte capacity.
  let read = unsafe { libc::proc_pidpath(pid as i32, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
  if read <= 0 {
    return Err(io::Error::last_os_error());
  }
  bytes.truncate(bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len()));
  Ok(PathBuf::from(OsString::from_vec(bytes)))
}
