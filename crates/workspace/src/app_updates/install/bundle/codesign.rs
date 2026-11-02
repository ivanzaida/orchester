//! What `codesign` says of a bundle: whether its signature holds, and the Team ID of the Developer ID that signed it.

use std::{io, path::Path, process::Command};

use super::run;

const CODESIGN: &str = "/usr/bin/codesign";
/// What `codesign --display` writes for the Team ID of a bundle signed ad hoc.
const NO_TEAM: &str = "not set";

/// Whether the new bundle `new` may replace `installed`: its signature verifies, deep and strict, and its Team ID is
/// the running bundle's.
pub(super) fn check(installed: &Path, new: &Path) -> io::Result<()> {
  run(Command::new(CODESIGN).args(["--verify", "--deep", "--strict"]).arg(new))
    .map_err(|error| io::Error::other(format!("the new app's code signature does not verify: {error}")))?;
  let running = team_id(installed)?.ok_or_else(|| io::Error::other("the running app has no Team ID"))?;
  match team_id(new)? {
    Some(team) if team == running => Ok(()),
    Some(team) => Err(io::Error::other(format!(
      "the new app is signed by the team {team}, not by this app's team {running}"
    ))),
    None => Err(io::Error::other("the new app has no Team ID")),
  }
}

/// The Team ID `bundle` is signed with; `None` when it is signed ad hoc.
pub(super) fn team_id(bundle: &Path) -> io::Result<Option<String>> {
  let details = run(Command::new(CODESIGN).args(["--display", "--verbose=2"]).arg(bundle))?;
  let team = details
    .lines()
    .find_map(|line| line.strip_prefix("TeamIdentifier="))
    .map(str::trim)
    .filter(|team| !team.is_empty() && *team != NO_TEAM);
  Ok(team.map(str::to_owned))
}
