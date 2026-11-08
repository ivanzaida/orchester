//! This computer's catalog for a shared project's web editor (`docs/architecture/server.md`, "Reporting", row 75):
//! the agent roles its connections have (CLI kinds) and the model ids per role, as core's [`sharing::catalog`] builds
//! them, with no connection or MCP server name. While the project's stream is open, the catalog is sent when it
//! connects and then read again every [`CATALOG_CHECK`], sent whenever it changed: a connection's role or a model
//! list. The body is the whole state, so a report that fails costs nothing: the next check sends it again, and offline
//! nothing waits.

use std::{convert::Infallible, path::Path, time::Duration};

use orchester_api::CatalogReport;
use orchester_core::{
  sharing::{self, LocalCatalog},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;

use super::stream::{Source, blocking};
use crate::sharing::SharingError;

/// How often an open stream reads the catalog again, so a change is reported at most once a minute.
pub const CATALOG_CHECK: Duration = Duration::from_secs(60);

/// Reports the catalog of the project at `root` through `source` at once, then every `every` when it changed since
/// the last report the server took. Runs until the stream ends and drops it.
pub(super) async fn keep_reported(source: &impl Source, root: &Path, every: Duration) -> Infallible {
  let mut sent: Option<CatalogReport> = None;
  loop {
    match blocking(root, read).await {
      Ok(report) if sent.as_ref() != Some(&report) => match source.report_catalog(report.clone()).await {
        Ok(()) => sent = Some(report),
        Err(error) => tracing::warn!(%error, "the catalog was not reported; the next check sends it again"),
      },
      Ok(_) => {}
      Err(error) => tracing::warn!(%error, "the catalog cannot be read"),
    }
    tokio::time::sleep(every).await;
  }
}

/// The catalog of the project at `root` as the server takes it.
fn read(root: &Path) -> Result<CatalogReport, SharingError> {
  let db = ProjectDb::open(root)?;
  Ok(report(&sharing::catalog(&db.connections()?)))
}

fn report(catalog: &LocalCatalog) -> CatalogReport {
  CatalogReport {
    cli_kinds: catalog.cli_kinds.iter().map(|kind| (*kind).to_owned()).collect(),
    models: catalog
      .models
      .iter()
      .map(|(role, ids)| ((*role).to_owned(), ids.clone()))
      .collect(),
  }
}
