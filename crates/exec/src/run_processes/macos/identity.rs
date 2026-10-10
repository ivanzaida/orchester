#![allow(unsafe_code)]
//! libproc's combined BSD/identity record, audit-token path lookup and signal APIs have no safe wrapper.
//! The kernel checks pidversion while holding a proc reference, which excludes exec; never fall back to kill(pid).
//! ABI: apple-oss-distributions/xnu: bsd/sys/proc_info_private.h and libsyscall/wrappers/libproc/libproc.h.

use std::{ffi::OsString, io, mem, os::unix::ffi::OsStringExt, path::PathBuf};

const PROC_FLAG_INEXIT: u32 = 4;

#[repr(C)]
pub(super) struct UniqueInfo {
  uuid: [u8; 16],
  pub(super) unique_id: u64,
  pub(super) parent_unique_id: u64,
  version: i32,
  original_parent_version: i32,
  reserved: [u64; 2],
}

#[repr(C)]
pub(super) struct ProcessInfo {
  pub(super) bsd: libc::proc_bsdinfo,
  pub(super) unique: UniqueInfo,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub(super) struct AuditToken {
  values: [u32; 8],
}

#[link(name = "proc")]
unsafe extern "C" {
  fn proc_pidpath_audittoken(token: *mut AuditToken, buffer: *mut libc::c_void, size: u32) -> i32;
  fn proc_signal_with_audittoken(token: *mut AuditToken, signal: i32) -> i32;
}

impl ProcessInfo {
  pub(super) fn token(&self) -> AuditToken {
    // These target APIs use PID and version only; signal permission comes from the caller's credentials.
    let mut values = [0; 8];
    values[5] = self.bsd.pbi_pid;
    values[7] = self.unique.version as u32;
    AuditToken { values }
  }

  pub(super) fn exited(&self) -> bool {
    self.bsd.pbi_status == 5 || self.bsd.pbi_flags & PROC_FLAG_INEXIT != 0
  }
}

pub(super) fn read(pid: u32) -> io::Result<Option<ProcessInfo>> {
  let pid = i32::try_from(pid).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid process id"))?;
  // SAFETY: both C records contain integers and byte arrays, all valid zeroed.
  let mut info: ProcessInfo = unsafe { mem::zeroed() };
  let size = mem::size_of::<ProcessInfo>() as i32;
  // SAFETY: flavor 18 (PROC_PIDT_BSDINFOWITHUNIQID) writes this exact C layout and size.
  let read = unsafe { libc::proc_pidinfo(pid, 18, 0, (&mut info as *mut ProcessInfo).cast(), size) };
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

impl AuditToken {
  pub(super) fn image(self) -> io::Result<Option<PathBuf>> {
    let mut token = self;
    let mut bytes = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: token has the audit_token_t layout; buffer holds the maximum documented path capacity.
    let read = unsafe { proc_pidpath_audittoken(&mut token, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
    if read <= 0 {
      let error = io::Error::last_os_error();
      if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(None);
      }
      return Err(error);
    }
    bytes.truncate(bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len()));
    Ok(Some(PathBuf::from(OsString::from_vec(bytes))))
  }

  pub(super) fn signal(self, signal: i32) -> io::Result<()> {
    let mut token = self;
    // SAFETY: the token is a complete audit_token_t. libproc returns errno directly, not -1.
    let error = unsafe { proc_signal_with_audittoken(&mut token, signal) };
    if error == 0 {
      Ok(())
    } else {
      // ESRCH may also mean exec, so callers must distinguish disappearance from a changed identity.
      Err(io::Error::from_raw_os_error(error))
    }
  }
}
