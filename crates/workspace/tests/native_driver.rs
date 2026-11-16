//! The native UI driver's state as the app shows it: ready, or the macOS permissions it misses.

use orchester_workspace::agent_testing::native_driver::{NativeDriverState, NativePermission, NativePermissions};

#[test]
fn the_state_names_each_missing_permission() {
  let both = |accessibility, screen_recording| {
    NativeDriverState::of(Some(NativePermissions {
      accessibility,
      screen_recording,
    }))
  };
  assert_eq!(both(true, true), NativeDriverState::Ready);
  assert_eq!(
    both(false, true),
    NativeDriverState::NeedsPermissions(vec![NativePermission::Accessibility])
  );
  assert_eq!(
    both(false, false),
    NativeDriverState::NeedsPermissions(vec![NativePermission::Accessibility, NativePermission::ScreenRecording])
  );
}

#[test]
fn a_platform_that_needs_no_permission_is_ready() {
  assert_eq!(NativeDriverState::of(None), NativeDriverState::Ready);
}

#[cfg(windows)]
#[test]
fn windows_is_ready_now() {
  assert_eq!(NativeDriverState::now(), NativeDriverState::Ready);
}
