#![allow(unsafe_code)]
//! libproc's combined BSD/identity record, audit-token path lookup and signal APIs have no safe wrapper.
//! ABI: apple-oss-distributions/xnu: bsd/sys/proc_info_private.h and libsyscall/wrappers/libproc/libproc.h.
//!
//! `proc_pidpath_audittoken` exists since macOS 11, the oldest the app supports, and is linked directly from
//! libSystem. `proc_signal_with_audittoken` first ships in macOS 15 (xnu-11215), so it is looked up at run time: a
//! direct reference would stop the app from loading on older systems. With it, the kernel checks pidversion while
//! holding a proc reference, which excludes exec. Without it, the token is read again immediately before
//! `kill(pid)`; the residual race is an exec, or an exit and PID reuse, in the moment between that read and the
//! signal.

use std::{ffi::OsString, io, mem, os::unix::ffi::OsStringExt, path::PathBuf, sync::OnceLock};

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

unsafe extern "C" {
  fn proc_pidpath_audittoken(token: *mut AuditToken, buffer: *mut libc::c_void, size: u32) -> i32;
}

/// `int proc_signal_with_audittoken(audit_token_t *, int)`; returns errno directly, not -1.
type SignalWithToken = unsafe extern "C" fn(*mut AuditToken, i32) -> i32;

/// The token-bound signal of macOS 15 and later, if this system has it.
fn signal_with_token() -> Option<SignalWithToken> {
  static FOUND: OnceLock<Option<SignalWithToken>> = OnceLock::new();
  *FOUND.get_or_init(|| {
    // SAFETY: a NUL-terminated symbol name looked up in the images already loaded; nothing is called here.
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"proc_signal_with_audittoken".as_ptr()) };
    if symbol.is_null() {
      return None;
    }
    // SAFETY: the symbol is libproc's function with exactly the C signature of `SignalWithToken`.
    Some(unsafe { mem::transmute::<*mut libc::c_void, SignalWithToken>(symbol) })
  })
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

  /// Signals the process this token names. ESRCH may also mean exec, so callers must distinguish disappearance
  /// from a changed identity.
  pub(super) fn signal(self, signal: i32) -> io::Result<()> {
    let Some(signal_with_token) = signal_with_token() else {
      return self.signal_after_check(signal);
    };
    let mut token = self;
    // SAFETY: the token is a complete audit_token_t and the function has the declared signature.
    let error = unsafe { signal_with_token(&mut token, signal) };
    if error == 0 {
      Ok(())
    } else {
      Err(io::Error::from_raw_os_error(error))
    }
  }

  /// Before macOS 15: the same answer as the token-bound signal, with the race the module comment names.
  fn signal_after_check(self, signal: i32) -> io::Result<()> {
    let pid = self.values[5];
    match read(pid)? {
      Some(current) if !current.exited() && current.token() == self => {}
      _ => return Err(io::Error::from_raw_os_error(libc::ESRCH)),
    }
    let pid = i32::try_from(pid).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid process id"))?;
    // SAFETY: kill takes only scalar arguments.
    if unsafe { libc::kill(pid, signal) } == 0 {
      Ok(())
    } else {
      Err(io::Error::last_os_error())
    }
  }
}
