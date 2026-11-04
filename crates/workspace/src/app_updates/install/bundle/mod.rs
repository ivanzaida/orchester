//! Replacing the app's bundle in place on macOS. The release's zip, which passed its size, SHA-256 and release
//! signature as it downloaded, is unpacked with `ditto` into a hidden folder beside the running bundle, on its volume
//! ([`unpack`]); the new bundle must pass `codesign --verify --deep --strict` and carry the running copy's Team ID
//! ([`codesign`]). A helper started outside the app's process group then waits for the app to exit, renames the old
//! bundle aside and the new one into its place — a rename on one volume, so never half done — opens it and removes
//! the old one; on any failure it puts the old bundle back, writes why to the result file the next start reads and
//! opens it again ([`helper`]). A file the app downloaded itself carries no quarantine attribute, so Gatekeeper lets
//! the new copy start without asking; none is added. The databases are in `~/Library/Application Support/Orchester`,
//! apart from the bundle, so the whole bundle is replaced.

mod codesign;
mod helper;
mod unpack;

use std::{
  io,
  path::Path,
  process::{Child, Command, Stdio},
};

use self::unpack::Unpacked;
use super::handover::Handover;
use crate::app_updates::{CopyReason, ThisCopy, copy::bundle::bundle_of};

/// `copy`, unless it is a bundle that would update itself but has no Team ID: `codesign` could then never match the
/// release's to it. Asked once as the app starts. Blocking.
pub(in crate::app_updates) fn signed_copy(copy: ThisCopy, folder: Option<&Path>) -> ThisCopy {
  let Some(bundle) = folder.and_then(bundle_of).filter(|_| copy == ThisCopy::Bundle) else {
    return copy;
  };
  match codesign::team_id(bundle) {
    Ok(Some(_)) => copy,
    Ok(None) => {
      tracing::info!(bundle = %bundle.display(), "the app's bundle has no Team ID; it does not update itself");
      ThisCopy::NotThisCopy(CopyReason::NoTeamId)
    }
    Err(error) => {
      tracing::warn!(%error, bundle = %bundle.display(), "codesign cannot tell the app's Team ID; it does not update itself");
      ThisCopy::NotThisCopy(CopyReason::NoTeamId)
    }
  }
}

/// Unpacks and checks the new bundle, and starts the helper that replaces the running one once the app exited; what
/// was unpacked goes when a step fails. Blocking.
pub(super) fn start(handover: &Handover) -> io::Result<Child> {
  let installed =
    bundle_of(&handover.program_folder).ok_or_else(|| io::Error::other("the app does not run from an .app bundle"))?;
  let unpacked = Unpacked::from_zip(&handover.installer, installed)?;
  let started = codesign::check(installed, &unpacked.app).and_then(|()| helper::start(handover, installed, &unpacked));
  if started.is_err() {
    unpacked.remove();
  }
  started
}

/// Runs `command` without input and gives what it wrote to standard error, where `ditto` and `codesign` write what
/// they found; an error with it, or with the exit status, when the command failed.
fn run(command: &mut Command) -> io::Result<String> {
  let output = command.stdin(Stdio::null()).output()?;
  let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
  if output.status.success() {
    return Ok(said);
  }
  if said.is_empty() {
    return Err(io::Error::other(output.status.to_string()));
  }
  Err(io::Error::other(said))
}
