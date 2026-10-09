//! `changes`: the `tasks` project with what the Changes tab reads (`Task · Changes · dark` `DYun5`) in every
//! form. From `tasks`: KON-12's checkpoint (4 files) and the work in its worktree since (2 files, one of them
//! new), KON-16 before its first checkpoint with a clean worktree, KON-9 ready for review with its worktree,
//! and KON-1 merged with its worktree removed. Added here: KON-25 whose checkpoint is too large to show,
//! KON-26 whose uncommitted work is, KON-27 with uncommitted work and no checkpoint, one of its lines too
//! long to show whole, and KON-28 whose worktree changed a checkpointed file again, so the file is in both
//! groups and counts once.

use orchester_core::task::organization::{Priority, WorkflowStatus};

use super::tasks;
use crate::kontur::{Kontur, SeedError, catalogue::Published, run::Drive};

/// One line past the most lines Orchester shows of a diff (`orchester_core::diff::MAX_DIFF_LINES`).
const TOO_MANY_LINES: usize = 20_001;
/// Repeats of a 61-character sentence in a line longer than Orchester shows whole
/// (`orchester_core::diff::MAX_LINE_CHARS`, 1,000).
const LONG_LINE_REPEATS: usize = 20;
const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = tasks::project(kontur)?;
  let feature = &catalogue.feature_delivery;
  variant(
    kontur,
    feature,
    ("Vendor the OpenAPI schema", "checkpoint too large to show"),
    |drive| {
      drive.checkpoint("implement", &[("schema/openapi.yaml", numbered(TOO_MANY_LINES))])?;
      Ok(())
    },
  )?;
  let fixtures = (
    "Generate fixtures for the importer",
    "uncommitted work too large to show",
  );
  variant(kontur, feature, fixtures, |drive| {
    drive
      .save_work("implement")?
      .uncommitted(&[("tests/fixtures/contacts.csv", numbered(TOO_MANY_LINES))])?;
    Ok(())
  })?;
  let long_line = format!(
    "// {}\n",
    "Each attempt is logged with its status, delay and endpoint. ".repeat(LONG_LINE_REPEATS)
  );
  let log = (
    "Format the delivery log",
    "no checkpoint, uncommitted work with a long line",
  );
  variant(kontur, feature, log, |drive| {
    drive.uncommitted(&[
      ("src/webhooks/format.rs", format!("{long_line}pub fn format() {{}}\n")),
      (
        "src/webhooks/mod.rs",
        "pub mod format;\npub mod log;\npub mod sender;\npub mod settings;\n".to_owned(),
      ),
    ])?;
    Ok(())
  })?;
  let again = ("Log every delivery attempt", "a checkpointed file changed again");
  variant(kontur, feature, again, |drive| {
    drive
      .checkpoint(
        "implement",
        &[("src/webhooks/attempts.rs", "pub struct Attempt;\n".to_owned())],
      )?
      .uncommitted(&[
        (
          "src/webhooks/attempts.rs",
          "pub struct Attempt {\n  pub number: u32,\n}\n".to_owned(),
        ),
        ("src/webhooks/attempts_log.rs", "pub fn record() {}\n".to_owned()),
      ])?;
    Ok(())
  })
}

/// A task titled `title` whose run works on Implement, with what `work` leaves in its worktree.
fn variant(
  kontur: &mut Kontur,
  feature: &Published,
  (title, state): (&str, &'static str),
  work: impl FnOnce(&mut Drive<'_>) -> Result<(), SeedError>,
) -> Result<(), SeedError> {
  let (created, started) = (kontur.ago(120), kontur.ago(60));
  let mut task = kontur.task((title, "Change it where it lives.", &CRITERIA), Some(feature), created)?;
  kontur.queue(&mut task, feature)?;
  kontur.organize(task.id(), (Priority::Medium, WorkflowStatus::InProgress), &[], None)?;
  let mut drive = Drive::start(kontur, &task, feature, started)?;
  drive.pass_before("implement")?.begin("implement")?;
  work(&mut drive)?;
  drop(drive);
  kontur.seeded(&task, state);
  Ok(())
}

/// `count` numbered lines.
fn numbered(count: usize) -> String {
  (1..=count).map(|line| format!("line {line}\n")).collect()
}
