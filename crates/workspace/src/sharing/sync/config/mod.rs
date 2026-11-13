//! A shared project's configuration on this computer (`docs/architecture/server.md`, "Shared configuration";
//! `docs/design/team-configuration-live.md`). The team's configuration is read from the server and held in memory only
//! ([`crate::team_config`]); nothing of it is kept in the project's database, where only what this computer fills in
//! for it is (connections and their picks, program paths, secret values, MCP switches, [`ShareFillIns`]). An admin's
//! change goes to the server at once (`crate::sharing::team_edit`). Share uploads the project's own configuration
//! ([`upload`]).

mod pipeline;
mod server;
mod share_fill_ins;
mod share_mcp;
pub(crate) mod team_body;
mod upload;

pub(crate) use self::pipeline::{pipeline_content, pipeline_executors};
pub use self::{
  server::ConfigServer,
  share_fill_ins::{FillInOutcome, ShareFillIns},
  upload::upload_configuration,
};
use super::stream::blocking;
