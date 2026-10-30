//! CI's release tool (`docs/stable-build.md`, "Releases from CI"): signs each release file with the release key as the
//! contract defines (`docs/cloud-api.md`, "The signed message") and uploads them to Orchester Cloud: creates the draft,
//! declares each file, sends its missing parts, and finalizes. The release counts as published only when the cloud
//! accepted every file.

mod error;
mod file;
mod upload;

pub use error::PublishError;
pub use file::ReleaseFile;
pub use upload::{Draft, create_draft, finalize, upload_file};
