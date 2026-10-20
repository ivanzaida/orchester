//! The `links` state (`Orchestrator · reply with links` `w262B`, `Orchestrator · links · states` `agv8r`): a
//! conversation whose replies link to places in Kontur and in Atlas, a second project registered beside it (its
//! folder is empty: only its name is read), to a pull request on the web, and to addresses the chat does not follow;
//! and a table of the runs, with links in its cells.

use std::{error::Error, fs, path::Path};

use orchester_core::{
  id::{ChatId, ConnectionId, ProjectId},
  name::Name,
  orchestrator::TurnState,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};

use super::data::{Part, Turn, ago, keep};

/// Atlas, registered in the app's database at `database` with its folder under `root`.
pub fn register_atlas(database: &Path, root: &Path) -> Result<ProjectId, Box<dyn Error>> {
  let atlas = Project::new(ProjectId::generate(), ProjectKey::new("ATL")?, Name::new("Atlas")?);
  let folder = root.join("repo-atlas");
  fs::create_dir_all(&folder)?;
  let mut db = AppDb::open(database)?;
  // The signed-in account's project, or the next one's to sign in.
  let owner = db.cloud_account()?.map(|account| account.account);
  db.register_project(&atlas, &folder, Timestamp::now(), owner.as_ref())?;
  Ok(atlas.id())
}

/// The conversation of project `project` with links, `(key, title)` its first task; `atlas` the other project.
pub fn conversation(
  db: &mut ProjectDb,
  (project, atlas): (ProjectId, ProjectId),
  (key, title): (&str, &str),
  connection: ConnectionId,
) -> Result<ChatId, Box<dyn Error>> {
  let task = format!("orchester:/project/{project}/tasks/{key}/details");
  let runs = format!(
    "Only Atlas has a run: [ATL-9 · Import schema](orchester:/project/{atlas}/tasks/ATL-9/details) is at 03 \
     Implement, 18 min in.\n\nIn Kontur, [{key} · {title}]({task}) is the only run going."
  );
  let status = format!(
    "{key} is ready for review. Its run finished, and the pull request waits for two checks.\n\n\
     - Task: [{key} · {title}]({task}) — Ready for review since 14:32\n\
     - Run: [Run #3 · Feature delivery](orchester:/project/{project}/tasks/{key}/conversation) — finished 14:32 · 7 \
     steps · 2.4M tokens\n\
     - Usage: [Usage · 1–15 Oct · {key} · by step](orchester:/project/{project}/usage?from=2026-10-01&to=2026-10-15&group=step) \
     — 6.1M tokens over 3 runs; 05 Review used the most\n\
     - Pull request: [#412 Local integration](https://github.com/kontur/kontur/pull/412) — 2 of 5 checks running · \
     review requested\n\n\
     The reviewer's notes are in [{key} › Conversation](orchester:/project/{project}/tasks/{key}/conversation). \
     [See the runbook](file:///H:/projects/kontur/docs/runbook.md) or the \
     [Status page](http://status.kontur.local)."
  );
  let table = format!(
    "Both runs, side by side:\n\n\
     | Task | Step | Tokens |\n\
     | --- | --- | --- |\n\
     | [{key} · {title}]({task}) | 05 Review | 2.4M |\n\
     | [ATL-9 · Import schema](orchester:/project/{atlas}/tasks/ATL-9/details) | 03 Implement | 0.8M |"
  );
  keep(
    db,
    ChatId::generate(),
    connection,
    vec![
      Turn {
        text: "Is anything running in my other projects?",
        at: ago(12),
        parts: vec![Part::Reply(&runs)],
        end: TurnState::Answered,
      },
      Turn {
        text: &format!("Where is {key}, what has it cost so far, and is its pull request through?"),
        at: ago(2),
        parts: vec![Part::Reply(&status)],
        end: TurnState::Answered,
      },
      Turn {
        text: "Compare the two runs.",
        at: ago(1),
        parts: vec![Part::Reply(&table)],
        end: TurnState::Answered,
      },
    ],
  )
}
