//! The app's icon where the system's shell looks for it beyond the window icon that lurq sets at the
//! window's creation ([`crate::brand::window_icon`]).
//!
//! On Windows the taskbar asks a window for its icon (`WM_GETICON`). When the window does not answer in
//! time it takes the icons of the window's class instead, and with none Windows' generic application
//! icon, which the taskbar button and its thumbnail keep. lurq's window class (registered by winit) has
//! no icons, and a development build does not answer for about a second while it draws its first frame.
//! [`install`], run once the first frame is presented, gives the class and the window the icon embedded
//! in the executable (`build.rs`) at the sizes Windows draws, from the frames of `orchester.ico` fitted
//! to the pixel grid, so every path shows Orchester's icon.
//!
//! On macOS the Dock takes an app's icon from its `.app` bundle (`Orchester App.app`, built by
//! `tools/package-macos.sh`), so there is nothing to give it. A build outside a bundle (`cargo run`) has
//! none: [`install`] says so, and the Dock shows a generic icon.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use self::windows::install;

#[derive(Debug, thiserror::Error)]
pub enum SystemIconError {
  #[error("cannot load the executable's icon at {size}px: {source}")]
  Load { size: i32, source: std::io::Error },
  #[error("this thread has no window with a window icon to give its class the app's icon")]
  NoWindow,
  #[error("cannot give the window class the app's icon: {0}")]
  SetClassIcon(std::io::Error),
  #[error("this platform shows the icon of an app bundle, and this program does not run from one")]
  NoAppBundle,
  #[error("cannot locate this program to tell whether it runs from an app bundle: {0}")]
  ProgramUnknown(std::io::Error),
}

/// Nothing to give: the Dock shows the bundle's icon. Reports a program outside a bundle, whose icon is
/// the generic one.
#[cfg(target_os = "macos")]
pub fn install() -> Result<(), SystemIconError> {
  let program = std::env::current_exe().map_err(SystemIconError::ProgramUnknown)?;
  if orchester_store::location::macos::in_app_bundle(&program) {
    Ok(())
  } else {
    Err(SystemIconError::NoAppBundle)
  }
}

/// Does nothing but report that the icon has to come from an app bundle.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn install() -> Result<(), SystemIconError> {
  Err(SystemIconError::NoAppBundle)
}
