//! A shared project's releases on the team-authority surface (`docs/architecture/server.md`, "Releases (team
//! authority)"; `docs/design/roadmap.md`): the release as the server keeps it under a revision, the writes of its
//! definition, *Mark released* with the moves it makes, every move of a task into or out of a release, and the
//! release's changelog. Which release a task is in is a field of the task ([`crate::TaskContent::release`]), changed
//! under the task's revision; a module's horizon is a field of the module ([`crate::ModuleFields::horizon`]).
//!
//! Not `crate::releases`, which is the app's own releases (its versions, files and signatures).

mod changelog;
mod release;

pub use changelog::*;
pub use release::*;
