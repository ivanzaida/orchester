//! What a team-authority execution bundle carries from core (S8, `docs/architecture/server.md`, "Execution
//! bundles"): the run's complete resolved inputs ([`CanonicalExecutionInputs`]), the files they name, and the
//! canonical bytes and hash server and desktop compute alike ([`canonical`]). The bundle itself, with what the server
//! adds, is the public API's `v2::ExecutionBundle`.

pub mod canonical;
mod error;
mod files;
mod inputs;
mod profile;

pub use self::{
  canonical::{CanonicalError, bundle_sha256, canonical_bytes, file_sha256},
  error::BundleError,
  files::{BundleFile, MAX_BUNDLE_FILE_BYTES, SkillFileEntry, SkillFiles},
  inputs::{CanonicalExecutionInputs, PinnedBodies, PinnedBody, ResolvedStepPermissions},
  profile::ExecutionProfile,
};
