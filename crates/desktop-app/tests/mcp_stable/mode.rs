//! Which builds listen on the fixed address: only the installed app, which refuses a discovery folder; every other
//! build takes a free port and writes a discovery file.

use std::ffi::OsString;

use orchester_app::mcp::{Build, INSTALLED_BUILD_VARIABLE};

use super::*;

#[test]
fn app_mcp_only_the_installed_app_uses_the_fixed_address_and_never_a_discovery_folder() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("a session");
  let folder = || Some(OsString::from(".tmp/x/mcp"));
  assert!(
    McpMode::choose(Build::Installed, folder(), Some(workspace)).is_none(),
    "the installed app started with a discovery folder runs without a server"
  );
  assert!(matches!(
    McpMode::choose(Build::Installed, None, Some(workspace)),
    Some(McpMode::Installation { .. })
  ));
  assert!(
    McpMode::choose(Build::Installed, None, None).is_none(),
    "no session, no address to read"
  );
  assert!(matches!(
    McpMode::choose(Build::Other, folder(), Some(workspace)),
    Some(McpMode::Qa(path)) if path == std::path::Path::new(".tmp/x/mcp")
  ));
  for workspace in [Some(workspace), None] {
    assert!(
      matches!(
        McpMode::choose(Build::Other, None, workspace),
        Some(McpMode::Development)
      ),
      "a build that is not the installed app never takes the fixed address or stores a token"
    );
  }
  assert_eq!(
    Build::this(),
    Build::Other,
    "tests are not built with {INSTALLED_BUILD_VARIABLE}"
  );
  launch.end();
}
