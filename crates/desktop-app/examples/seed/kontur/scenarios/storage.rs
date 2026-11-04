//! Storage fixtures use real disk reads: an intentionally high Keep free holds the guard on QA's disk.
use orchester_core::{
  store::{RunStore, SettingsStore, WorkspaceStore},
  workspaces::{Borrow, Holder, KeepFree},
};

use crate::kontur::{
  Kontur, SeedError,
  shapes::{self, Shape},
};

pub fn low_disk(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.quick_fix;
  for (index, (title, shape)) in [
    ("Add retry to webhook sender", Shape::Running("implement")),
    ("Webhook endpoint settings", Shape::Cancelled("implement")),
    ("Rotate API keys", Shape::Succeeded),
  ]
  .into_iter()
  .enumerate()
  {
    let created = kontur.ago(120 + index as i64 * 60);
    let mut task = kontur.task(
      (
        title,
        "Implement the change and cover it with tests.",
        &["The tests pass."],
      ),
      Some(pipeline),
      created,
    )?;
    shapes::apply(kontur, &mut task, Some(pipeline), shape, created)?;
  }
  kontur
    .db
    .save_keep_free(KeepFree::from_gigabytes(KeepFree::MAX_GB).expect("the maximum is valid"))?;
  for run in kontur.db.running_runs()? {
    let mut run = run.map_err(|bad| bad.error)?;
    let events = run.pause_for_low_disk(kontur.ago(1))?;
    kontur.db.append_events(&run, &events)?;
    let holder = Holder::Run { run: run.id() };
    let Borrow::Take(number) = kontur
      .db
      .borrow_workspace(holder, None, std::num::NonZeroU32::MIN, kontur.ago(60))?
    else {
      return Err(SeedError::Invalid("the fixture workspace is free".into()));
    };
    let path = number.path(&kontur.repo);
    std::fs::create_dir_all(&path).map_err(|source| SeedError::Io {
      path: path.clone(),
      source,
    })?;
    std::fs::write(path.join("warm-build.bin"), vec![0_u8; 1024]).map_err(|source| SeedError::Io { path, source })?;
    kontur.db.return_workspace(holder, kontur.ago(30))?;
  }
  Ok(())
}
pub fn nothing(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::empty::seed(kontur)
}
