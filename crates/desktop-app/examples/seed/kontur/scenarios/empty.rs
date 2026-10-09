//! `empty`: the project with its connection, agent profiles and pipelines, and no task
//! (`Tasks · empty project · dark` `PMJrB`, `Overview · empty project · dark` `a5CcQx`).

use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue().map(drop)
}
