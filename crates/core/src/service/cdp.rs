//! The `cdp` UI driver's rules (`docs/architecture/agent-testing.md`, "UI drivers"): an Electron or WebView2 app is
//! driven over the Chrome DevTools Protocol through the remote debugging port it listens on, the port Orchester leased
//! for the service, `{{port}}`. An Electron app takes it as an argument the service's arguments include
//! (`--remote-debugging-port={{port}}`); a WebView2 app takes it from [`WEBVIEW2_ARGUMENTS`], which Orchester sets for
//! the service's process on Windows ([`webview2_arguments`]). WKWebView on macOS has no such port: there the driver
//! answers [`CdpUnsupported`], which points to the native driver.

use thiserror::Error;

use super::ServiceName;
use crate::connection::invocation::Platform;

/// The variable a WebView2 app's runtime reads extra browser arguments from, the debugging port among them.
pub const WEBVIEW2_ARGUMENTS: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

/// Why the `cdp` driver cannot reach a service on this platform.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error(
  "the service `{0}` uses the cdp UI driver, which reaches Electron and WebView2 apps through their remote debugging \
   port; WKWebView on macOS has none: use the native driver for this service on macOS"
)]
pub struct CdpUnsupported(pub ServiceName);

/// Whether the `cdp` driver reaches the service `name` on `platform`: not on macOS.
pub fn cdp_supported(platform: Platform, name: &ServiceName) -> Result<(), CdpUnsupported> {
  match platform {
    Platform::Windows => Ok(()),
    Platform::MacOs => Err(CdpUnsupported(name.clone())),
  }
}

/// The browser argument that makes an app's Chromium listen for CDP on `port` of the loopback interface.
pub fn debugging_port_argument(port: u16) -> String {
  format!("--remote-debugging-port={port}")
}

/// The value of [`WEBVIEW2_ARGUMENTS`] for a service whose debugging port is `port`: `own`, the value the service's
/// variables set, if any, with the port's argument after it.
pub fn webview2_arguments(own: Option<&str>, port: u16) -> String {
  let argument = debugging_port_argument(port);
  match own.map(str::trim).filter(|own| !own.is_empty()) {
    Some(own) => format!("{own} {argument}"),
    None => argument,
  }
}
