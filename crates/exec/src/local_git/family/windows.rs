#![allow(unsafe_code)]
//! std has no job-object or suspended-primary-thread wrapper. A private job is assigned before any child code runs.
//! Closing the final job handle kills its descendants, including writers whose original parent has exited.
use std::{
  mem,
  os::windows::{
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::CommandExt,
  },
  process::{Child, Command},
};

use orchester_core::{blob::ContentHash, id::IdempotencyKey, integration::local::ProcessIdentity};
use windows_sys::Win32::{
  Foundation::{ERROR_FILE_NOT_FOUND, HANDLE, INVALID_HANDLE_VALUE},
  System::{
    Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next},
    JobObjects::{
      AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
      JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
      JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation, OpenJobObjectW,
      QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    },
    Threading::{
      CREATE_NO_WINDOW, CREATE_SUSPENDED, GetProcessIdOfThread, OpenThread, ResumeThread,
      THREAD_QUERY_LIMITED_INFORMATION, THREAD_SUSPEND_RESUME,
    },
  },
};

use super::super::{LocalError, LocalGitError, digest};

pub(in crate::local_git) struct Family {
  handle: OwnedHandle,
  token: ContentHash,
}
impl Family {
  pub(in crate::local_git) fn create() -> Result<Self, LocalGitError> {
    let token = digest(IdempotencyKey::generate().to_string().as_bytes());
    let name = name(&token);
    // SAFETY: name is NUL-terminated; default security and no preexisting object is expected for a fresh token.
    let handle = owned(unsafe { CreateJobObjectW(std::ptr::null(), name.as_ptr()) })?;
    // SAFETY: the C record has valid zero initialization.
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: the live job handle and the correctly-sized C record remain valid through the call.
    if unsafe {
      SetInformationJobObject(
        handle.as_raw_handle(),
        JobObjectExtendedLimitInformation,
        (&raw const limits).cast(),
        mem::size_of_val(&limits) as u32,
      )
    } == 0
    {
      return Err(LocalError::ProcessContainmentUnsupported.into());
    }
    Ok(Self { handle, token })
  }
  pub(in crate::local_git) fn spawn(&self, command: &mut Command) -> Result<Child, LocalGitError> {
    command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|_| LocalGitError::Process)?;
    // SAFETY: both retained handles are live; the suspended process has not run or created descendants.
    if unsafe { AssignProcessToJobObject(self.handle.as_raw_handle(), child.as_raw_handle()) } == 0 {
      child.kill().map_err(|_| LocalError::ProcessUncertain)?;
      child.wait().map_err(|_| LocalError::ProcessUncertain)?;
      return Err(LocalError::ProcessContainmentUnsupported.into());
    }
    Ok(child)
  }
  pub(in crate::local_git) fn identity(&self, child: &Child) -> Result<ProcessIdentity, LocalGitError> {
    let mut identity = crate::run_processes::identity::capture(child.id())
      .map_err(|_| LocalError::ProcessUncertain)?
      .ok_or(LocalError::ProcessUncertain)?;
    identity.containment = Some(self.token.clone());
    let executor = crate::run_processes::identity::capture(std::process::id())
      .map_err(|_| LocalError::ProcessUncertain)?
      .ok_or(LocalError::ProcessUncertain)?;
    identity.executor = Some(orchester_core::integration::local::ExecutorIdentity {
      pid: executor.pid,
      started: executor.started,
    });
    Ok(identity)
  }
  pub(in crate::local_git) fn resume(&self, child: &Child) -> Result<(), LocalGitError> {
    // SAFETY: the flags request a thread snapshot and the PID argument is ignored.
    let snapshot = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    // SAFETY: the C record permits zero initialization; dwSize identifies its size to ToolHelp.
    let mut entry: THREADENTRY32 = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of_val(&entry) as u32;
    // SAFETY: the snapshot and writable C record are valid.
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    while found {
      if entry.th32OwnerProcessID == child.id() {
        // SAFETY: the retained suspended child prevents PID reuse; no child code has created another thread.
        let thread = owned(unsafe {
          OpenThread(
            THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION,
            0,
            entry.th32ThreadID,
          )
        })?;
        // SAFETY: the thread handle is live and the retained child prevents reuse of its process PID.
        if unsafe { GetProcessIdOfThread(thread.as_raw_handle()) } != child.id() {
          return Err(LocalError::ProcessUncertain.into());
        }
        // SAFETY: this handle belongs to the suspended primary thread of our retained child.
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
          return Err(LocalGitError::Process);
        }
        return Ok(());
      }
      // SAFETY: the same snapshot and writable record remain valid.
      found = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    }
    Err(LocalError::ProcessUncertain.into())
  }
  pub(in crate::local_git) fn empty(&self) -> Result<bool, LocalGitError> {
    empty(&self.handle)
  }
  pub(in crate::local_git) fn stop(&self) -> Result<(), LocalGitError> {
    // SAFETY: only the private job assigned by this executor is terminated; no foreign PID is signalled.
    if unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) } == 0 {
      return Err(LocalError::ProcessUncertain.into());
    }
    Ok(())
  }
}
fn name(token: &ContentHash) -> Vec<u16> {
  format!("Local\\orchester-local-{}", token.as_str())
    .encode_utf16()
    .chain([0])
    .collect()
}
fn owned(handle: HANDLE) -> Result<OwnedHandle, LocalGitError> {
  if handle.is_null() || handle == INVALID_HANDLE_VALUE {
    return Err(LocalError::ProcessUncertain.into());
  }
  // SAFETY: the successful API returned a uniquely owned handle.
  Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}
fn empty(handle: &OwnedHandle) -> Result<bool, LocalGitError> {
  // SAFETY: the C record has valid zero initialization.
  let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { mem::zeroed() };
  // SAFETY: the retained job and correctly-sized writable C record remain valid.
  if unsafe {
    QueryInformationJobObject(
      handle.as_raw_handle(),
      JobObjectBasicAccountingInformation,
      (&raw mut info).cast(),
      mem::size_of_val(&info) as u32,
      std::ptr::null_mut(),
    )
  } == 0
  {
    return Err(LocalError::ProcessUncertain.into());
  }
  Ok(info.ActiveProcesses == 0)
}
pub(in crate::local_git) fn stopped(identity: &ProcessIdentity) -> Result<bool, LocalGitError> {
  let token = identity.containment.as_ref().ok_or(LocalError::ProcessUncertain)?;
  let name = name(token);
  // SAFETY: the persisted random private name is NUL-terminated; opening requests only observation.
  let raw = unsafe { OpenJobObjectW(4, 0, name.as_ptr()) };
  if raw.is_null() {
    return if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) {
      // A kill-on-close job disappears only once all its members have exited and handles are gone.
      Ok(true)
    } else {
      Err(LocalError::ProcessUncertain.into())
    };
  }
  empty(&owned(raw)?)
}
