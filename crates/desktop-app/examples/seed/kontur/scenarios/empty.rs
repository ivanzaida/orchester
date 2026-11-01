//! `empty`: the project with its connection, agent profiles and pipelines, and no task
//! (`Tasks · empty project · dark` `PMJrB`, `Overview · empty project · dark` `a5CcQx`). `join-team` writes the same:
//! for it the QA app adds a stand-in team server with an invite (`--join-team`, `tools/qa-app.ps1`), for *Join a
//! team* (`drCQd`, `aWsVx`, `oOn6j`).

use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue().map(drop)
}
