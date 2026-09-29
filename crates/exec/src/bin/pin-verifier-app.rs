//! Copy an MCP-enabled desktop and its runner into a run-owned directory before UI verification.

use std::{
  env,
  io::{self, Write},
  path::PathBuf,
};

use orchester_exec::verifier_app::{PinError, VerifierPlatform, pin_verifier_app};

fn main() -> Result<(), PinError> {
  let mut args = env::args_os().skip(1);
  let source_dir = args.next().map(PathBuf::from).ok_or(PinError::Usage)?;
  let run_dir = args.next().map(PathBuf::from).ok_or(PinError::Usage)?;
  if args.next().is_some() {
    return Err(PinError::Usage);
  }
  let app = pin_verifier_app(&source_dir, &run_dir, VerifierPlatform::host()?)?;
  writeln!(io::stdout().lock(), "{}", app.desktop().display()).map_err(PinError::Output)
}
