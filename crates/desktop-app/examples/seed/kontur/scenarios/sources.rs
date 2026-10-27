//! `sources`: folders and files outside the worktree that agents may read without asking (`Sources` `XulGo`). The
//! project lists a folder of docs, a crate's sources and a runbook that is missing; Feature delivery's next version
//! lists design tokens for every step and a style guide on its Review step; a draft on Feature delivery lists a spec
//! of its own. They are made beside the repository, outside it. `sources-unreadable` damages the project's list.

use std::{collections::BTreeMap, fs, path::Path};

use orchester_core::{
  resource::{Draft, Pipeline, PipelineSources},
  source::SourcePath,
  store::ResourceStore,
};

use crate::kontur::{Kontur, SeedError, io_error};

/// The step of Feature delivery whose agent gets the style guide.
const REVIEW: &str = "Review";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let beside = kontur
    .repo
    .parent()
    .ok_or_else(|| SeedError::Invalid("the repository has no parent folder".to_owned()))?
    .to_path_buf();
  let docs = folder(
    &beside.join("kontur-docs"),
    &["webhooks.md", "public-api.md", "retries.md"],
  )?;
  let tokio = folder(&beside.join("tokio-1.48.0"), &["lib.rs", "time.rs"])?;
  let tokens = folder(&beside.join("design-tokens"), &["tokens.json"])?;
  let style = folder(&beside.join("kontur-style-guide"), &["review.md", "naming.md"])?;
  let spec = beside.join("webhooks-v2.md");
  fs::write(&spec, "# Webhooks v2\n").map_err(io_error(&spec))?;
  // Listed, then gone: runs start without it.
  let runbook = beside.join("kontur-ops").join("runbook.md");
  kontur
    .db
    .set_project_sources(&[source(&docs)?, source(&tokio)?, source(&runbook)?])?;
  let id = catalogue.feature_delivery.id();
  let latest = kontur
    .db
    .latest_version::<Pipeline>(id)?
    .expect("the catalogue publishes Feature delivery");
  let review = latest
    .body()
    .steps()
    .iter()
    .find(|step| step.name.as_str() == REVIEW)
    .ok_or_else(|| SeedError::NoSuchStep(REVIEW.to_owned()))?
    .key
    .clone();
  let sources = PipelineSources::new(
    vec![source(&tokens)?],
    BTreeMap::from([(review, vec![source(&style)?])]),
  );
  let body = latest
    .body()
    .clone()
    .with_sources(sources)
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur
    .db
    .save_draft(id, &Draft::new(body, Some(latest.number()), kontur.now))?;
  kontur.db.publish_draft::<Pipeline>(id, kontur.now)?;
  let task = kontur.task(
    (
      "Retry failed webhook deliveries",
      "Retry a delivery that failed, with backoff, as the webhooks v2 spec says.",
      &["A failed delivery is retried with backoff."],
    ),
    Some(&catalogue.feature_delivery),
    kontur.ago(30),
  )?;
  kontur.db.set_task_sources(task.id(), &[source(&spec)?])?;
  kontur.seeded(&task, "draft with a source");
  Ok(())
}

/// `sources-unreadable`: what `sources` writes, with a relative path among the project's sources, as an edit outside
/// Orchester would leave it. No read takes it, so the store calls the list corrupt: the step and pipeline Sources
/// pages say why they cannot be read (`Step sources · the project's can't be read` `niNNR` in `g6Ecy`).
pub fn unreadable(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  // The store has no API for a damaged list.
  let changed = rusqlite::Connection::open(kontur.database())?
    .execute("INSERT INTO project_sources (position, path) VALUES (99, 'docs')", [])?;
  if changed != 1 {
    return Err(SeedError::Invalid(format!(
      "damaging the project's sources changed {changed} rows"
    )));
  }
  Ok(())
}

/// The folder `path`, made with `files` in it.
fn folder(path: &Path, files: &[&str]) -> Result<std::path::PathBuf, SeedError> {
  fs::create_dir_all(path).map_err(io_error(path))?;
  for file in files {
    let file = path.join(file);
    fs::write(&file, "fixture\n").map_err(io_error(&file))?;
  }
  Ok(path.to_path_buf())
}

fn source(path: &Path) -> Result<SourcePath, SeedError> {
  SourcePath::new(&path.to_string_lossy()).map_err(|error| SeedError::Invalid(error.to_string()))
}
