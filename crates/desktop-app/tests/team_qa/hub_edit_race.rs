//! Saves of an admin's draft of a team MCP server that run on the blocking pool without being awaited in order
//! (ORC-836b review): an editor starts one for every change of its fields, and two can be on the pool at once. A save
//! that started earlier must never write after a later one, or the draft of what is typed now is replaced by older
//! text, or thrown away when the older fields equal the version's own; and the text *Publish for the team* sends is the
//! last typed.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_hub_edit_race`

use std::{sync::Arc, thread, time::Duration};

use orchester_core::id::ResourceId;
use orchester_workspace::{
  agents::{
    save_lane::SaveLane,
    team_mcp_edit::{self, Saved},
  },
  app_mcp::setup::{NewTeamMcpServer, Runs},
};

use super::{
  hub_edit::{draft_of, on_pencil_until},
  team::State,
};

/// `published` with `arguments` as its program's arguments.
fn with_arguments(published: &NewTeamMcpServer, arguments: &[&str]) -> NewTeamMcpServer {
  let mut fields = published.clone();
  if let Runs::Program { arguments: kept, .. } = &mut fields.runs {
    *kept = arguments.iter().map(|argument| (*argument).to_owned()).collect();
  }
  fields
}

fn published_fields(root: &std::path::Path, id: ResourceId) -> NewTeamMcpServer {
  team_mcp_edit::published(root, id)
    .expect("read")
    .expect("a server the form holds")
}

#[test]
fn team_qa_hub_edit_race_a_late_stale_autosave_never_undoes_a_newer_draft() {
  let (window, _server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  let published = published_fields(&root, pencil);
  let typed = with_arguments(&published, &["--typed"]);
  let lane = SaveLane::default();
  // The save of the fields the form opened with started first; the one of the typed fields started after it and ends
  // first.
  let stale = lane.start();
  let newest = lane.start();

  let wrote = lane.run_if_newest(newest, || team_mcp_edit::save(&root, pencil, &typed));
  assert!(matches!(wrote, Some(Ok(Saved::Draft(_)))), "{wrote:?}");
  let late = lane.run_if_newest(stale, || team_mcp_edit::save(&root, pencil, &published));

  assert!(late.is_none(), "an out-of-date save writes nothing");
  assert_eq!(draft_of(&window, pencil), Some(vec!["--typed".to_owned()]));
}

#[test]
fn team_qa_hub_edit_race_saves_that_overlap_on_threads_leave_the_newest_draft() {
  let (window, _server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  let published = published_fields(&root, pencil);
  let typed = with_arguments(&published, &["--typed"]);
  for round in 0..20_u64 {
    let lane = Arc::new(SaveLane::default());
    let (stale, newest) = (lane.start(), lane.start());
    let (old_lane, old_root, old_fields) = (lane.clone(), root.clone(), published.clone());
    let (new_lane, new_root, new_fields) = (lane, root.clone(), typed.clone());
    // Either of them may reach the pool first.
    let (old_wait, new_wait) = if round % 2 == 0 { (2, 0) } else { (0, 2) };
    let old = thread::spawn(move || {
      thread::sleep(Duration::from_millis(old_wait));
      old_lane.run_if_newest(stale, || team_mcp_edit::save(&old_root, pencil, &old_fields))
    });
    let new = thread::spawn(move || {
      thread::sleep(Duration::from_millis(new_wait));
      new_lane.run_if_newest(newest, || team_mcp_edit::save(&new_root, pencil, &new_fields))
    });
    new
      .join()
      .expect("the newest save ran")
      .expect("the newest save is never skipped")
      .expect("saved");
    old.join().expect("the stale save ran");
    assert_eq!(
      draft_of(&window, pencil),
      Some(vec!["--typed".to_owned()]),
      "round {round}"
    );
    team_mcp_edit::discard(&root, pencil).expect("discarded");
  }
}

#[test]
fn team_qa_hub_edit_race_publish_saves_the_last_typed_text_after_any_older_save() {
  let (window, _server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  let published = published_fields(&root, pencil);
  let older = with_arguments(&published, &["--older"]);
  let last = with_arguments(&published, &["--last"]);
  let lane = SaveLane::default();
  // An autosave of older text started before *Publish for the team* was pressed and has not reached the database.
  let stale = lane.start();

  let saved = lane.run_newest(|| team_mcp_edit::save(&root, pencil, &last));
  let late = lane.run_if_newest(stale, || team_mcp_edit::save(&root, pencil, &older));

  assert!(matches!(saved, Ok(Saved::Draft(_))), "{saved:?}");
  assert!(late.is_none(), "the older autosave writes nothing after the press");
  assert_eq!(draft_of(&window, pencil), Some(vec!["--last".to_owned()]));
}
