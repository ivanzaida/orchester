//! Platforms without the native driver yet (macOS until its Accessibility driver): every call answers so.

use orchester_core::orchester_tools::ui::{UiAction, UiRef};

use super::{NativeCapture, NativeSnapshot, NativeUiError, NativeWindow};

pub fn find_windows(_pids: &[u32]) -> Result<Vec<NativeWindow>, NativeUiError> {
  Err(NativeUiError::NotSupported)
}

pub fn open(_windows: &[NativeWindow]) -> Result<Vec<NativeWindow>, NativeUiError> {
  Err(NativeUiError::NotSupported)
}

pub fn snapshot(_windows: &[NativeWindow], _query: Option<&str>) -> Result<NativeSnapshot, NativeUiError> {
  Err(NativeUiError::NotSupported)
}

pub fn act(_windows: &[NativeWindow], _target: &UiRef, _action: &UiAction) -> Result<(), NativeUiError> {
  Err(NativeUiError::NotSupported)
}

pub fn capture(_windows: &[NativeWindow], _target: Option<&UiRef>) -> Result<NativeCapture, NativeUiError> {
  Err(NativeUiError::NotSupported)
}

pub fn idle(_windows: &[NativeWindow], _timeout_ms: u32) -> Result<(), NativeUiError> {
  Err(NativeUiError::NotSupported)
}
