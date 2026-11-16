//! The notes the Orchestrator's runs-on picker shows under its efforts only in a state of the project's: a draft
//! already saved in the Orchestrator's settings (`orchestrator-over-draft`: *Choosing here … replaces the draft*), and
//! a publish the checks refuse (`orchestrator-publish-refused`: *Not saved: …*).

use orchester_core::orchestrator::OrchestratorSettings;

use super::{MODEL, accounts::CodexPlan, model, project};
use crate::kontur::{Kontur, SeedError};

/// `orchestrator` with a draft saved over the published version (Claude · Work, the same model, a lower effort).
pub fn over_draft(kontur: &mut Kontur) -> Result<(), SeedError> {
  let (sign_ins, _) = project(kontur, CodexPlan::Used)?;
  let published = OrchestratorSettings::new(sign_ins.work, model(MODEL)?, Some("high".to_owned()));
  kontur.db.publish_orchestrator(&published, kontur.ago(3 * 24 * 60))?;
  let draft = OrchestratorSettings::new(sign_ins.work, model(MODEL)?, Some("low".to_owned()));
  kontur.db.save_orchestrator_draft(&draft, kontur.ago(60))?;
  Ok(())
}

/// `orchestrator`, except that the project's database refuses a new version of the Orchestrator's settings (a trigger
/// stands for a check that fails), so choosing anything in the picker is refused and the picker says so.
pub fn publish_refused(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::seed(kontur)?;
  rusqlite::Connection::open(kontur.database())?.execute_batch(
    "CREATE TRIGGER qa_refuse_orchestrator_version BEFORE INSERT ON orchestrator_versions
     BEGIN SELECT RAISE(ABORT, 'refused by the checks'); END;",
  )?;
  Ok(())
}
