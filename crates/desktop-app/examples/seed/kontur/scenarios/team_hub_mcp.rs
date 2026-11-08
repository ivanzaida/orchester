//! `team-hub-mcp-*`: the Agents hub of `team-hub` with the panel of a team MCP server in each state of `Agents · MCP
//! server states` (`acFkz`, C4–C8, ORC-833). Pick MCP servers in the filter, then the server:
//!
//! - C4 `SpRvd` *No program path yet*: `team-hub`, the server `pencil-design`.
//! - C5 `h2md5` *Path set, file missing*: `team-hub-mcp-path-gone`, `pencil-design`.
//! - C6 `LiS1F` *A needed secret without a value*: `team-hub-mcp-secret`, `pencil-design` (its path is a file here and
//!   its variable `PENCIL_TOKEN` has no value).
//! - C7 `tV4LG` *Remote server with a secret header*: `team-hub`, `github`.
//! - C8 `lSA4x` *Ready here*: `team-hub-mcp-ready`, `figma` (a program, `npx`, with its path set).
//!
//! No scenario puts a secret value in the keychain, which the seed does not touch: a state that shows a value set is
//! reached in the app by *Add value*, a person's press.

use std::fs;

use orchester_core::{id::ResourceId, time::Timestamp};
use orchester_store::AppDb;

use super::team_hub::{Mcp, seed_with};
use crate::kontur::{Kontur, SeedError};

/// `team-hub-mcp-path-gone`: the path of `pencil-design` was set, and its file is not there.
pub fn path_gone(kontur: &mut Kontur) -> Result<(), SeedError> {
  let ids = seed_with(kontur, &Mcp::default())?;
  let gone = kontur.repo.join(".orchester").join("qa-bin").join("pencil-mcp");
  set_program(kontur, ids["pencil-design"], &gone, 6)
}

/// `team-hub-mcp-secret`: `pencil-design` has its program (a file here) and takes the secret `PENCIL_TOKEN`, which has
/// no value.
pub fn secret(kontur: &mut Kontur) -> Result<(), SeedError> {
  let ids = seed_with(
    kontur,
    &Mcp {
      pencil_variables: &["PENCIL_TOKEN"],
      figma_program: false,
    },
  )?;
  let program = program_file(kontur, "pencil-mcp")?;
  set_program(kontur, ids["pencil-design"], &program, 3)
}

/// `team-hub-mcp-ready`: `figma` runs `npx`, whose path is set to a file here, and is on.
pub fn ready(kontur: &mut Kontur) -> Result<(), SeedError> {
  let ids = seed_with(
    kontur,
    &Mcp {
      pencil_variables: &[],
      figma_program: true,
    },
  )?;
  let program = program_file(kontur, "npx")?;
  set_program(kontur, ids["figma"], &program, 3)
}

/// An empty file `name` in the QA folder of programs, which a path can lead to.
fn program_file(kontur: &Kontur, name: &str) -> Result<std::path::PathBuf, SeedError> {
  let path = kontur.repo.join(".orchester").join("qa-bin").join(name);
  fs::write(&path, "").map_err(|error| SeedError::Invalid(error.to_string()))?;
  Ok(path)
}

/// Keeps `program` as the path of the team MCP server `server` on this computer, set `days` ago.
fn set_program(kontur: &Kontur, server: ResourceId, program: &std::path::Path, days: i64) -> Result<(), SeedError> {
  let link = kontur
    .db
    .sharing()?
    .ok_or_else(|| SeedError::Invalid("the scenario's project is not shared".to_owned()))?;
  let at: Timestamp = kontur.ago(days * 24 * 60);
  AppDb::open(&kontur.app_database)?.set_mcp_server_program(link.server, server, program, at)?;
  Ok(())
}
