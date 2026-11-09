//! Saves of an admin's draft of a team MCP server that run on the blocking pool without being awaited in order
//! (ORC-836b review): an editor starts one for every change of its fields, and two can be on the pool at once. A save
//! that started earlier must never write after a later one, or the draft of what is typed now is replaced by older
//! text, or thrown away when the older fields equal the version's own; and the text *Publish for the team* sends is the
//! last typed.
//!
//! The lane that orders them belongs to the resource, not to the panel, and the last typed text is written even when
//! the panel is gone a frame after the typing; a save the store refused is tried again, and *Publish for the team*
//! sends what was typed whatever an older save is doing.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_hub_edit_race`

use std::{
  sync::{Arc, mpsc},
  thread::{self, JoinHandle},
  time::{Duration, Instant},
};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{TeamMcpServer, VersionNumber},
  store::ResourceStore,
};
use orchester_workspace::{
  agents::{
    save_lane::SaveLane,
    team_mcp_edit::{self, Saved},
  },
  app_mcp::setup::{NewTeamMcpServer, Runs},
};

use super::{
  hub_edit::{ARGUMENTS, PUBLISH, db, draft_of, on_pencil_until},
  team::State,
};

/// A save that holds a lane's turn until it is released, as a slow disk would: every save started meanwhile waits.
struct Stalled {
  release: mpsc::Sender<()>,
  holder: JoinHandle<()>,
}

impl Stalled {
  fn on(lane: &Arc<SaveLane>) -> Self {
    let (entered, started) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let lane = lane.clone();
    let holder = thread::spawn(move || {
      lane.run_newest(|| {
        entered.send(()).expect("the test waits");
        released.recv().expect("the test releases");
      });
    });
    started.recv().expect("the save holds the lane");
    Self { release, holder }
  }

  fn release(self) {
    self.release.send(()).expect("the holder waits");
    self.holder.join().expect("the holder ended");
  }
}

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

  let wrote = lane.run_if_newest(newest, || {
    team_mcp_edit::save(&root, pencil, &typed, orchester_core::resource::VersionNumber::FIRST)
  });
  assert!(matches!(wrote, Some(Ok(Saved::Draft(_)))), "{wrote:?}");
  let late = lane.run_if_newest(stale, || {
    team_mcp_edit::save(
      &root,
      pencil,
      &published,
      orchester_core::resource::VersionNumber::FIRST,
    )
  });

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
      old_lane.run_if_newest(stale, || {
        team_mcp_edit::save(
          &old_root,
          pencil,
          &old_fields,
          orchester_core::resource::VersionNumber::FIRST,
        )
      })
    });
    let new = thread::spawn(move || {
      thread::sleep(Duration::from_millis(new_wait));
      new_lane.run_if_newest(newest, || {
        team_mcp_edit::save(
          &new_root,
          pencil,
          &new_fields,
          orchester_core::resource::VersionNumber::FIRST,
        )
      })
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

  let saved =
    lane.run_newest(|| team_mcp_edit::save(&root, pencil, &last, orchester_core::resource::VersionNumber::FIRST));
  let late = lane.run_if_newest(stale, || {
    team_mcp_edit::save(&root, pencil, &older, orchester_core::resource::VersionNumber::FIRST)
  });

  assert!(matches!(saved, Ok(Saved::Draft(_))), "{saved:?}");
  assert!(late.is_none(), "the older autosave writes nothing after the press");
  assert_eq!(draft_of(&window, pencil), Some(vec!["--last".to_owned()]));
}

#[test]
fn team_qa_hub_edit_race_the_lane_outlives_the_panel_so_a_stale_save_of_a_closed_panel_loses() {
  let (window, _server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  let servers = window.launch.services().expect("the session").servers.clone();
  let published = published_fields(&root, pencil);
  let typed = with_arguments(&published, &["--typed"]);
  let before = draft_of(&window, pencil);
  let (first_panel, second_panel) = (servers.save_lane(&root, pencil), servers.save_lane(&root, pencil));
  assert!(
    Arc::ptr_eq(&first_panel, &second_panel),
    "every panel of the resource shares one lane"
  );

  // The first panel's save of the fields it opened with is on the pool and stalled; the panel is closed, a second one
  // opens and what is typed there is saved: it must come after the stalled one, which would undo it.
  let stale = first_panel.start();
  let (entered, started) = mpsc::channel();
  let (release, released) = mpsc::channel::<()>();
  let stale_root = root.clone();
  let stalled = thread::spawn(move || {
    first_panel.run_if_newest(stale, || {
      entered.send(()).expect("the test waits");
      released.recv().expect("the test releases");
      team_mcp_edit::save(
        &stale_root,
        pencil,
        &published,
        orchester_core::resource::VersionNumber::FIRST,
      )
    })
  });
  started.recv().expect("the stale save holds the lane");
  let newest = second_panel.start();
  let newer = thread::spawn(move || {
    second_panel.run_if_newest(newest, || {
      team_mcp_edit::save(&root, pencil, &typed, orchester_core::resource::VersionNumber::FIRST)
    })
  });
  thread::sleep(Duration::from_millis(50));
  assert_eq!(
    draft_of(&window, pencil),
    before,
    "the newer save waits for the lane's turn"
  );
  release.send(()).expect("the stale save waits");
  stalled.join().expect("the stale save ran");
  newer
    .join()
    .expect("the newer save ran")
    .expect("it was not skipped")
    .expect("saved");

  assert_eq!(draft_of(&window, pencil), Some(vec!["--typed".to_owned()]));
}

#[test]
fn team_qa_hub_edit_race_the_last_typed_text_is_written_when_the_panel_is_gone_a_frame_later() {
  let (mut window, _server, (project, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  let servers = window.launch.services().expect("the session").servers.clone();
  let stalled = Stalled::on(&servers.save_lane(&root, pencil));
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  // Two keystrokes, then the person leaves before the newest save's answer was awaited: the panel is unmounted.
  window.set(ARGUMENTS, "--first");
  window.set(ARGUMENTS, "--last");
  window.navigation().go(Route::Project(project, ProjectPage::Review));
  window.until("the panel to go", |window| window.texts("team-hub-panel").is_none());
  stalled.release();

  window.until("the last text in the draft", |window| {
    draft_of(window, pencil) == Some(vec!["--last".to_owned()])
  });
}

#[test]
fn team_qa_hub_edit_race_publish_sends_what_was_typed_while_an_older_save_is_stalled() {
  let (mut window, server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  let servers = window.launch.services().expect("the session").servers.clone();
  let before = draft_of(&window, pencil);
  let stalled = Stalled::on(&servers.save_lane(&root, pencil));
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  window.set(ARGUMENTS, "--last");
  // Pressed at once: no autosave has written anything, and the press takes its turn in the lane behind the stalled
  // save, so nothing is sent from a draft an older save may still change.
  window.press(PUBLISH);
  let pressed = Instant::now();
  window.until("the press to wait its turn", |_| {
    pressed.elapsed() > Duration::from_millis(400)
  });
  let numbered = || {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "pencil-design" && versions.len() > 1)
  };
  assert!(!numbered(), "nothing is sent while an older save holds the lane");
  assert_eq!(draft_of(&window, pencil), before, "and nothing was written meanwhile");
  stalled.release();

  window.until("the server to number it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "pencil-design" && versions.len() == 2)
  });
  let sent = server
    .resources("kontur")
    .into_iter()
    .find(|(resource, _)| resource["name"] == "pencil-design")
    .map(|(_, versions)| format!("{:?}", versions.last()))
    .expect("the resource");
  assert!(sent.contains("--last"), "{sent}");
}

/// Makes the project's database refuse writes, or lets it write again.
#[cfg(unix)]
fn writable(root: &std::path::Path, yes: bool) {
  use std::os::unix::fs::PermissionsExt;
  let folder = root.join(".orchester");
  for entry in std::fs::read_dir(&folder).expect("the project's folder") {
    let path = entry.expect("an entry").path();
    if path.is_file() {
      let mode = if yes { 0o644 } else { 0o444 };
      std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("permissions");
    }
  }
}

#[cfg(unix)]
#[test]
fn team_qa_hub_edit_race_a_newest_save_the_store_refused_is_told_and_tried_again() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  let root = window.data.repo("repo-kontur");
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  writable(&root, false);
  window.set(ARGUMENTS, "--typed");
  window.until("the refusal told", |window| {
    window.texts("team-hub-edit-save-error").is_some()
  });
  writable(&root, true);

  // Nothing is typed again: the save is tried again by itself.
  window.until_within(Duration::from_secs(15), "the draft kept after the retry", |window| {
    draft_of(window, pencil) == Some(vec!["--typed".to_owned()])
  });
}

#[test]
fn team_qa_hub_edit_race_an_autosave_queued_before_rebase_never_overwrites_the_merged_draft() {
  let (window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-conflict-clean", State::TeamEditConflictClean),
    "team-hub-compare",
  );
  let root = window.data.repo("repo-kontur");
  let servers = window.launch.services().expect("the session").servers.clone();
  let lane = servers.save_lane(&root, pencil);
  let v2 = VersionNumber::FIRST.next().expect("a second version");
  let stale_fields = with_arguments(&published_fields(&root, pencil), &["--stale"]);
  let before_rebase = draft_of(&window, pencil);
  // The person's version met the team's v2 and waits in the outbox: the rebase takes it back as the draft.
  let outbox = db(&window).waiting_publishes().expect("outbox")[0].version;

  // An autosave of the fields typed before the press is queued behind a save that is slow to finish.
  let stalled = Stalled::on(&lane);
  let ticket = lane.start();
  let queued = {
    let root = root.clone();
    thread::spawn(move || {
      lane.run_if_newest(ticket, || {
        team_mcp_edit::save(&root, pencil, &stale_fields, VersionNumber::FIRST)
      })
    })
  };
  let rebasing = {
    let handle = window.data.runtime.handle().clone();
    thread::spawn(move || handle.block_on(servers.rebase_mcp_draft(&root, pencil, (Some(outbox), v2), Vec::new())))
  };
  thread::sleep(Duration::from_millis(200));
  assert_eq!(
    draft_of(&window, pencil),
    before_rebase,
    "the rebase takes its turn in the lane, behind the save that holds it"
  );
  stalled.release();

  let based = rebasing.join().expect("the rebase ran").expect("rebased");
  assert!(
    queued.join().expect("the autosave ran").is_none(),
    "the rebase started after the autosave, which is out of date from then on"
  );
  assert_eq!(based.number, v2);
  let kept = db(&window)
    .draft::<TeamMcpServer>(pencil)
    .expect("read")
    .expect("the merged draft");
  assert_eq!(
    kept.based_on(),
    Some(v2),
    "the draft is the merge, not the autosave's text"
  );
  assert_eq!(
    draft_of(&window, pencil),
    Some(
      ["--stdio", "--app", "desktop", "--port", "3845"]
        .map(str::to_owned)
        .to_vec()
    ),
    "the merge of the person's version with the team's"
  );
}
