//! New pipeline in the Pipelines hub (`Pipelines · v2` P14 `Rszkt`, P14a `d4f9Vi`, P14b `sVNK4`, P14c `g09dL`): the
//! pipelines of P8 (*Dependency update* under *Needs you*; *Feature delivery*, *Quick fix* and *Release notes* ready),
//! and of the profiles and tools the starter looks for only *Explorer*, *Builder*, *Run test suite* and *Create pull
//! request*, so a new pipeline's starter has 02 *Plan* and 05 *Review* with no profile, amber with *Choose profile*.
//! *New pipeline* in the head opens P14 and P14c; the name *Quick fix* gives P14a; *Duplicate* on *Feature delivery*
//! gives P14b. No agent or command is started.

use super::{
  pipelines::profile,
  pipelines_not_ready::{dependency_update, published},
};
use crate::kontur::{Kontur, SeedError, catalogue::command_tool};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let explorer = profile(kontur, "Explorer", &["cargo test"], true)?;
  let builder = profile(kontur, "Builder", &["cargo test"], true)?;
  command_tool(&mut kontur.db, "Run test suite", &["cargo", "test"], kontur.now)?;
  command_tool(
    &mut kontur.db,
    "Create pull request",
    &["gh", "pr", "create", "--fill"],
    kontur.now,
  )?;
  published(
    kontur,
    "Feature delivery",
    &[("Research", explorer), ("Implement", builder), ("Review", builder)],
  )?;
  published(kontur, "Quick fix", &[("Fix", builder), ("Check", builder)])?;
  published(kontur, "Release notes", &[("Write", builder)])?;
  dependency_update(kontur, builder)
}
