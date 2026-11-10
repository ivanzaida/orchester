//! `orchestrator-another-account`: `orchestrator` with one more conversation, the latest, whose agent session lives on
//! *Claude · Work*'s account, the one the Orchestrator runs on. The rail's tooltip of *Claude · Personal* says that
//! another account starts a new session, and so does the chip's once the Orchestrator is run on *Personal*
//! (`Another account` `vsqci` in `Runs-on picker · states` `t7cdi`).

use orchester_core::orchestrator::{Chat, OrchestratorSettings, TurnState};

use super::{MODEL, Part, accounts::CodexPlan, model, project, turn};
use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let (sign_ins, _) = project(kontur, CodexPlan::Used)?;
  let settings = OrchestratorSettings::new(sign_ins.work, model(MODEL)?, Some("medium".to_owned()));
  kontur.db.publish_orchestrator(&settings, kontur.ago(3 * 24 * 60))?;
  let start = kontur.ago(2);
  let chat = Chat::named(orchester_core::id::ChatId::generate(), "What runs?", start);
  turn(
    kontur,
    &chat,
    ("What runs?", start),
    vec![Part::Reply("Nothing runs now.")],
    &TurnState::Answered,
  )?;
  kontur
    .db
    .set_chat_session(chat.id(), "session-1", (sign_ins.work, sign_ins.work_account))?;
  Ok(())
}
