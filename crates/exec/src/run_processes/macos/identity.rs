#![allow(unsafe_code)]
//! libproc's combined BSD/identity record, audit-token path lookup and signal APIs have no safe wrapper.
//! ABI: apple-oss-distributions/xnu: bsd/sys/proc_info_private.h and libsyscall/wrappers/libproc/libproc.h.
//! `sysctl(KERN_PROC_PID)` fills bsd/sys/sysctl.h's `kinfo_proc`, which libc does not describe; only its first field,
//! the start time, is read.
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

/// `sizeof(struct kinfo_proc)` on 64-bit macOS; its first field, `kp_proc.p_un.__p_starttime`, is a `timeval`.
const KINFO_PROC_SIZE: usize = 648;

/// The start time of `pid` in microseconds from `sysctl(KERN_PROC_PID)`, which still describes a process that exited
/// and waits to be reaped, as `proc_pidinfo` no longer does; `None` once it is gone.
pub(crate) fn started_even_exited(pid: u32) -> io::Result<Option<u64>> {
  let pid = i32::try_from(pid).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid process id"))?;
  let mut name = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
  let mut record = [0u8; KINFO_PROC_SIZE];
  let mut length = record.len();
  // SAFETY: the name holds its stated 4 integers; the buffer is writable for `length` bytes, which sysctl updates to
  // what it wrote; no new value is set.
  let result = unsafe {
    libc::sysctl(
      name.as_mut_ptr(),
      4,
      record.as_mut_ptr().cast(),
      &mut length,
      std::ptr::null_mut(),
      0,
    )
  };
  if result != 0 {
    return Err(io::Error::last_os_error());
  }
  match length {
    0 => Ok(None),
    KINFO_PROC_SIZE => {
      // SAFETY: the buffer holds a whole kinfo_proc, whose first bytes are a timeval; it may be unaligned.
      let start: libc::timeval = unsafe { std::ptr::read_unaligned(record.as_ptr().cast()) };
      let micros = u64::try_from(start.tv_sec)
        .ok()
        .and_then(|seconds| seconds.checked_mul(1_000_000))
        .zip(u64::try_from(start.tv_usec).ok())
        .and_then(|(seconds, micros)| seconds.checked_add(micros));
      micros
        .map(Some)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process start time out of range"))
    }
    _ => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "unexpected process record size",
    )),
  }
}
