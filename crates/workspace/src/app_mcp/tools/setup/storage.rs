//! `get_storage` and `set_keep_free` (ORC-422): the disk guard as Project settings › Runs › Storage reads and sets
//! it. The guard holds in every project, warm workspaces on or off; the free space it keeps is no permission, so
//! lowering and raising it are both taken. The runner reads it at its next check: no restart.

use orchester_core::{
  store::SettingsStore,
  workspaces::{CleanableKind, DiskUsage, GIGABYTE, KeepFree},
};
use orchester_exec::disk_space::LocalDisks;
use orchester_store::ProjectDb;
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::{Spec, parse, projects::InProject};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  storage::{self, CleanableRow, DiskGuard},
  tasks::outside::Outside,
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "get_storage",
    description: "A project's disk guard, as Project settings › Runs › Storage reads it: disk_guard (keep_free_gb, \
                  the free space it keeps on the repository's disk, 50 by default; free_gb and total_gb of that disk, \
                  null when it cannot be read; holds, whether it holds work back now) and, while it holds, cleanable: \
                  the folders that could be cleaned to make room, in the order to clean — the worktrees of ended runs \
                  first (kind worktree, with run, task_key and run_number), then idle warm workspaces (kind \
                  workspace, with number), each the largest first — with path and size_bytes (null when it could not \
                  be read). While it holds, no step starts and working steps pause with their work committed; they go \
                  on by themselves once there is room. It holds in every project, warm workspaces on or off. Sizing \
                  walks the folders, so it can take a while while the guard holds.",
    schema: schema_for_type::<InProject>,
    read_only: true,
  },
  Spec {
    name: "set_keep_free",
    description: "Sets the free space, in whole GB, the project's disk guard keeps on the repository's disk, as \
                  Project settings › Runs › Storage does: below it no step starts and working steps pause until there \
                  is room. Lowering and raising it are both taken; the runner applies it at its next check without a \
                  restart. Refused as invalid_keep_free outside 1 to 1048576. Returns the disk guard as get_storage \
                  gives it, without the cleanable list.",
    schema: schema_for_type::<SetKeepFree>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetKeepFree {
  /// The project's id, or its name.
  pub project: String,
  /// Whole GB to keep free, 1 to 1048576.
  pub gigabytes: u64,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(services: &Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  match name {
    "get_storage" => get_storage(services, parse(arguments)?).await,
    "set_keep_free" => set_keep_free(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn get_storage(services: &Services, args: InProject) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let read = storage::read_local(&open.repo_root)?;
    let cleanable: Vec<Value> = read.cleanable.iter().map(cleanable_json).collect();
    Ok(json!({ "disk_guard": guard_json(&read.guard), "cleanable": cleanable }))
  })
  .await
}

async fn set_keep_free(services: &Services, args: SetKeepFree) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let keep = KeepFree::from_gigabytes(args.gigabytes).map_err(|_| {
      let reason = Message::new("appMcp.errors.invalidKeepFree")
        .with("min", KeepFree::MIN_GB.to_string())
        .with("max", KeepFree::MAX_GB.to_string());
      ToolError::refused("invalid_keep_free", vec![reason])
    })?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    db.save_keep_free(keep)?;
    services.changes.record(open.project.id(), Outside::ProjectSettings);
    let guard = DiskGuard::read(&db, &open.repo_root, &LocalDisks)?;
    Ok(json!({ "disk_guard": guard_json(&guard) }))
  })
  .await
}

/// The disk guard as the tools give it, in whole GB.
pub(super) fn guard_json(guard: &DiskGuard) -> Value {
  let gigabytes = |bytes: u64| bytes / GIGABYTE;
  json!({
    "keep_free_gb": guard.keep_free.gigabytes(),
    "free_gb": guard.disk.map(|DiskUsage { free, .. }| gigabytes(free)),
    "total_gb": guard.disk.map(|DiskUsage { total, .. }| gigabytes(total)),
    "holds": guard.holds(),
  })
}

fn cleanable_json(row: &CleanableRow) -> Value {
  let path = row.cleanable.path.display().to_string();
  let size = row.cleanable.bytes;
  match row.cleanable.kind {
    CleanableKind::EndedRunWorktree { run } => json!({
      "kind": "worktree",
      "run": run.to_string(),
      "task_key": row.run.as_ref().map(|(key, _)| key),
      "run_number": row.run.as_ref().map(|(_, number)| number),
      "path": path,
      "size_bytes": size,
    }),
    CleanableKind::IdleWorkspace { number } => json!({
      "kind": "workspace",
      "number": number.get(),
      "path": path,
      "size_bytes": size,
    }),
  }
}
