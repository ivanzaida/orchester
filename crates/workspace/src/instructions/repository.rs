//! Import from repository on the Instructions side: the same scan as the Skills side, whose instruction drafts are
//! named by their file's stem; repository paths remain intact.
pub use crate::skills::repository::{CreatedDraft, RepositoryScan, ScanError, create_drafts_in, scan};
