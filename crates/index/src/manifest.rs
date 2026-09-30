//! `manifest.json`: what an index holds and how it was built. An index is reused only when its
//! manifest reads back with the current [`FORMAT`].

use std::{collections::BTreeMap, fs, path::Path};

use orchester_core::{revision::Revision, time::Timestamp};
use serde::{Deserialize, Serialize};

use crate::{IndexError, digest::Digests, outline, record::FileRecord};

/// The version of the index files. An index of another version, or made by other grammars
/// ([`generator`]), is built again, since its files may differ from what this build writes for the
/// same revision.
pub(crate) const FORMAT: u32 = 3;
const FILE: &str = "manifest.json";

/// How an index was built.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildMode {
  Full,
  /// From the index of `from`, reading only the `changed` paths that differ.
  Incremental {
    from: Revision,
    changed: usize,
  },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LanguageCount {
  pub(crate) files: usize,
  pub(crate) lines: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Manifest {
  pub(crate) format: u32,
  pub(crate) generator: String,
  pub(crate) revision: Revision,
  pub(crate) built_at: Timestamp,
  pub(crate) build_ms: u64,
  pub(crate) build: BuildMode,
  pub(crate) files: usize,
  pub(crate) lines: usize,
  pub(crate) symbols: usize,
  pub(crate) outlines: usize,
  pub(crate) doc_headings: usize,
  pub(crate) packages: usize,
  pub(crate) languages: BTreeMap<String, LanguageCount>,
  /// The digest of each of the index's other files.
  pub(crate) digests: Digests,
}

/// The counts of `records`, for a manifest.
pub(crate) struct Counts {
  pub(crate) files: usize,
  pub(crate) lines: usize,
  pub(crate) symbols: usize,
  pub(crate) outlines: usize,
  pub(crate) doc_headings: usize,
  pub(crate) languages: BTreeMap<String, LanguageCount>,
}

impl Counts {
  pub(crate) fn of(records: &[FileRecord]) -> Self {
    let mut languages: BTreeMap<String, LanguageCount> = BTreeMap::new();
    for record in records {
      let count = languages.entry(record.language.name().to_owned()).or_default();
      count.files += 1;
      count.lines += record.lines.unwrap_or(0);
    }
    Self {
      files: records.len(),
      lines: languages.values().map(|count| count.lines).sum(),
      symbols: records.iter().map(|record| record.symbols.len()).sum(),
      outlines: records.iter().filter(|record| record.outline.is_some()).count(),
      doc_headings: records.iter().map(|record| record.headings.len()).sum(),
      languages,
    }
  }
}

impl Manifest {
  pub(crate) fn write(&self, dir: &Path) -> Result<(), IndexError> {
    let file = dir.join(FILE);
    let text = serde_json::to_string_pretty(self).expect("a manifest serializes to JSON");
    fs::write(&file, text).map_err(IndexError::io("write", file))
  }

  /// The manifest of the index in `dir`, when there is one of the current format. Why another
  /// one does not read changes nothing: that index is built again.
  pub(crate) fn read_current(dir: &Path) -> Option<Self> {
    let text = fs::read_to_string(dir.join(FILE)).ok()?;
    serde_json::from_str::<Self>(&text)
      .ok()
      .filter(|manifest| manifest.format == FORMAT && manifest.generator == generator())
  }
}

/// What made an index: this crate's version and the grammars it parses with.
pub(crate) fn generator() -> String {
  format!("orchester-index {}; {}", env!("CARGO_PKG_VERSION"), outline::grammars())
}
