//! This computer's catalog for a shared project's web editor (`docs/architecture/server.md`, "Reporting", row 75):
//! its agent connections and MCP servers by kind, name and role, and the model ids per connection role, as core's
//! [`sharing::catalog`] builds them. While the project's stream is open, the catalog is sent when it connects and then
//! read again every [`CATALOG_CHECK`], sent whenever it changed: a connection, an MCP server or a model list. The body
//! is the whole state, so a report that fails costs nothing: the next check sends it again, and offline nothing waits.

use std::{convert::Infallible, path::Path, time::Duration};

use orchester_api::{CatalogKind, CatalogReference, CatalogReport};
use orchester_core::{
  sharing::{self, CatalogEntryKind, LocalCatalog},
  store::{ConnectionStore, McpServerStore},
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
  Ok(report(&sharing::catalog(&db.connections()?, &db.mcp_servers()?)))
}

fn report(catalog: &LocalCatalog) -> CatalogReport {
  CatalogReport {
    references: catalog
      .entries
      .iter()
      .map(|entry| CatalogReference {
        kind: match entry.kind {
          CatalogEntryKind::Connection => CatalogKind::Connection,
          CatalogEntryKind::McpServer => CatalogKind::McpServer,
        },
        name: entry.name.clone(),
        role: entry.role.map(str::to_owned),
      })
      .collect(),
    models: catalog
      .models
      .iter()
      .map(|(role, ids)| ((*role).to_owned(), ids.clone()))
      .collect(),
  }
}
