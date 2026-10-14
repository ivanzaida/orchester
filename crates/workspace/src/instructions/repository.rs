//! Instructions import names come from the source's stem; repository paths remain intact.
use std::path::Path;

use orchester_core::{agent_files::import::Candidate, name::Name};

pub use crate::skills::repository::{CreatedDraft, RepositoryScan, ScanError, create_drafts_in};

pub fn scan(repo_root: &Path) -> Result<RepositoryScan, ScanError> {
  let mut scan = crate::skills::repository::scan(repo_root)?;
  for item in std::sync::Arc::make_mut(&mut scan.items) {
    if let Ok(Candidate::Instruction { name, .. }) = &mut item.outcome
      && let Some(stem) = name.as_str().strip_suffix(".md")
    {
      *name = Name::new(stem).expect("recognized root instruction filenames have a nonempty stem");
    }
  }
  Ok(scan)
}
