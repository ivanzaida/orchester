//! What the hub's panel of a team MCP server says of a version published here that the team server has not numbered,
//! and what the press that ends it does (`Agents · v2` A13 `WMtxm` result, A14 `gTLx6`, A15 `dLqHC`, A16 `x3ywL`, A17
//! `rJbPm`, ORC-836): the QA result states `team-edit-published`, `team-edit-refused-admin`, `team-edit-waiting`,
//! `team-edit-rejected` and `team-create-name-taken` are what a person's press leaves behind, made without one. A
//! version the server refused or that met another one is the panel's to show, with its way out; no edit goes on top of
//! it; a press that cannot be taken says why in place; and one send of a project's outbox at a time, so *Cancel
//! sending* never deletes a version a send of it is carrying to the server.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_hub_edit`

use std::time::{Duration, Instant};

use orchester_app::test_support::Window;
use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpServer, VersionNumber},
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::PublishState;
use orchester_workspace::{
  app_mcp::setup::{NewTeamMcpServer, Runs},
  sharing::team_edit::{EditRefused, Edited},
};

use super::{
  hub_edit::{ARGUMENTS, PUBLISH, db, draft_of, on_pencil_until},
  team::State,
};

/// The panel's chip of the team's version, which every state of the panel has.
const TEAM_CHIP: &str = "team-hub-chip-team";

#[test]
fn team_qa_hub_edit_published_shows_the_version_the_server_numbered() {
  let (mut window, server, (_, pencil)) = on_pencil_until(("team-edit-published", State::TeamEditPublished), TEAM_CHIP);
  window.until("the team's version", |window| window.text(TEAM_CHIP) == "Team · v2");
  assert!(window.texts("team-hub-edit").is_some(), "an admin may edit it again");
  assert!(window.texts(PUBLISH).is_none() && window.texts("team-hub-edit-waiting").is_none());
  assert!(draft_of(&window, pencil).is_none(), "the draft became the version");
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
  assert!(
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "pencil-design" && versions.len() == 2),
    "the server numbered it"
  );
}

#[test]
fn team_qa_hub_edit_refused_admin_is_read_only_with_the_draft_kept_and_discard_goes() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-refused-admin", State::TeamEditRefusedAdmin),
    "team-hub-edit-not-admin-title",
  );
  assert_eq!(
    window.text("team-hub-edit-not-admin-title"),
    "You are no longer an admin of the team",
    "this computer knows it, the server did not just say it"
  );
  assert!(window.texts(PUBLISH).is_none() && window.texts(ARGUMENTS).is_none());
  assert!(window.texts("team-hub-edit").is_none(), "no Edit for a member");
  assert!(draft_of(&window, pencil).is_some(), "the draft stays on this computer");
  window.press("team-hub-edit-discard");
  window.until("the draft to go", |window| draft_of(window, pencil).is_none());
}

#[test]
fn team_qa_hub_edit_waiting_state_cancels_back_to_the_draft() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-waiting", State::TeamEditWaiting),
    "team-hub-edit-waiting-title",
  );
  assert!(
    window.texts("team-hub-edit").is_none(),
    "nothing is edited on top of a version that waits"
  );
  window.press("team-hub-edit-cancel");
  window.until("the draft to come back", |window| window.texts(PUBLISH).is_some());
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
  assert!(draft_of(&window, pencil).is_some());
}

#[test]
fn team_qa_hub_edit_cancel_sending_says_why_it_cannot_be_taken_back() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-waiting", State::TeamEditWaiting),
    "team-hub-edit-waiting-title",
  );
  // The person is no longer an admin on this computer: taking the version back as their draft is refused.
  let mut database = db(&window);
  database
    .record_sharing_state(Some(MemberRole::Member), Some(Timestamp::now()), None)
    .expect("the role");
  database.set_server_admin(false).expect("not the server's admin");
  window.press("team-hub-edit-cancel");
  window.until("the reason", |window| {
    window.texts("team-hub-edit-action-error").is_some()
  });
  let said = window.text("team-hub-edit-action-error");
  assert!(
    said.contains("Only the project's owner or the server's admin"),
    "{said}"
  );
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "it still waits"
  );
  assert!(draft_of(&window, pencil).is_none());
}

#[test]
fn team_qa_hub_edit_a_rejected_version_in_the_outbox_is_the_refused_form_and_fixing_it_publishes_again() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  assert_eq!(
    window.text("team-hub-edit-refused-title"),
    "The team's server refused v2",
    "red, as the design says it, and not the waiting bar"
  );
  let why = window.text("team-hub-edit-refused-body");
  assert!(why.contains("does not accept its content"), "{why}");
  assert!(window.texts("team-hub-edit-refused-status").is_none());
  assert!(window.texts("team-hub-edit").is_none(), "no Edit on top of it");
  assert!(window.texts(PUBLISH).is_some() && window.texts("team-hub-edit-discard").is_some());
  assert!(
    window.texts(ARGUMENTS).is_some(),
    "the refused version is a form to fix"
  );
  assert!(window.text("team-hub-actions-hint").starts_with("Refused "));
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox")[0].state,
    PublishState::Refused {
      code: "invalid:body".to_owned()
    }
  );

  window.set(ARGUMENTS, "--fixed");
  window.press(PUBLISH);
  // The server still rejects the content: the version is the draft again, with what was typed, and the reason stays.
  window.until("the refusal in place, the version the draft again", |window| {
    db(window).waiting_publishes().expect("outbox").is_empty()
      && draft_of(window, pencil) == Some(vec!["--fixed".to_owned()])
      && window.texts("team-hub-edit-refused-title").is_some()
      && window.texts(PUBLISH).is_some()
  });
}

#[test]
fn team_qa_hub_edit_a_refused_form_publishes_when_it_differs_from_the_teams_version_not_from_the_refused_one() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  // Typing the team's own arguments back is no change to publish: the hint says it would only revert.
  let team = db(&window)
    .version::<TeamMcpServer>(pencil, VersionNumber::FIRST)
    .expect("read")
    .expect("the team's v1");
  let team_arguments: Vec<String> = match serde_json::to_value(team.body()).expect("a body")["launch"]["args"].clone() {
    serde_json::Value::Array(args) => args.iter().filter_map(|arg| arg.as_str().map(str::to_owned)).collect(),
    _ => Vec::new(),
  };
  window.set(ARGUMENTS, &team_arguments.join("\n"));
  window.until("the hint to say it would revert", |window| {
    window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").contains("only revert v2"))
  });
  window.press(PUBLISH);
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "nothing was sent: the refused version is still in the outbox"
  );
  // The refused version's own body, unedited, differs from the team's: it publishes again (and is refused again here).
  window.set(ARGUMENTS, "--stdio\n--app\ndesktop\n--no-telemetry");
  window.until("the hint to say fix and publish again", |window| {
    window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").contains("fix the reason, then publish again"))
  });
  window.press(PUBLISH);
  window.until("the version to be the draft again", |window| {
    db(window).waiting_publishes().expect("outbox").is_empty() && draft_of(window, pencil).is_some()
  });
}

#[test]
fn team_qa_hub_edit_a_refused_form_keeps_what_is_typed_as_the_draft() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  window.set(ARGUMENTS, "--typed-here");
  window.until("the draft in the database", |window| {
    draft_of(window, pencil) == Some(vec!["--typed-here".to_owned()])
  });
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "the refused version stays in the outbox until it is published again or discarded"
  );
}

#[test]
fn team_qa_hub_edit_discarding_a_rejected_version_goes_back_to_the_team_s_version() {
  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  window.press("team-hub-edit-discard");
  window.until("nothing of it kept", |window| {
    db(window).waiting_publishes().expect("outbox").is_empty() && draft_of(window, pencil).is_none()
  });
  window.until("the plain panel on the team's version", |window| {
    window.texts("team-hub-edit").is_some() && window.texts(PUBLISH).is_none() && window.text(TEAM_CHIP) == "Team · v1"
  });
  // Edit starts again from the team's version, not from the refused one.
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  let arguments = window.text(ARGUMENTS);
  assert!(!arguments.contains("--no-telemetry"), "{arguments}");
}

#[test]
fn team_qa_hub_edit_the_list_says_the_outbox_first_and_what_the_computer_needs_after_it() {
  let row = |id: ResourceId| format!("team-hub-row-{id}-line");
  let (mut window, _server, (_, pencil)) = on_pencil_until(("team-edit-waiting", State::TeamEditWaiting), TEAM_CHIP);
  window.until("the row", |window| window.texts(&row(pencil)).is_some());
  assert_eq!(window.text(&row(pencil)), "Waiting to send · v2");
  assert_eq!(window.text(TEAM_CHIP), "Team · v1");
  // Without its program path the row says what is missing after it.
  let services = window.launch.services().expect("the session");
  let root = window.data.repo("repo-kontur");
  services
    .workspace
    .clear_team_mcp_program(&root, pencil)
    .expect("cleared");
  window.until("the need after the outbox", |window| {
    window.text(&row(pencil)) == "Waiting to send · v2 · set its program path on this computer"
  });

  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-refused-admin", State::TeamEditRefusedAdmin),
    "team-hub-edit-not-admin-title",
  );
  window.until("the row", |window| window.texts(&row(pencil)).is_some());
  assert_eq!(window.text(&row(pencil)), "Your draft on v1 · read-only here");

  let (mut window, _server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  window.until("the row", |window| window.texts(&row(pencil)).is_some());
  assert_eq!(window.text(&row(pencil)), "Your draft on v1 · publish refused");
}

/// The top of the element `id` in the window, when it is laid out.
fn top(window: &mut Window, id: &str) -> Option<f32> {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .map(|bounds| bounds.y)
}

#[test]
fn team_qa_hub_edit_a_version_that_waits_or_was_refused_is_listed_under_needs_you_here() {
  for (scenario, state, shown) in [
    (
      "team-edit-waiting",
      State::TeamEditWaiting,
      "team-hub-edit-waiting-title",
    ),
    (
      "team-edit-rejected",
      State::TeamEditRejected,
      "team-hub-edit-refused-title",
    ),
    (
      "team-edit-refused-admin",
      State::TeamEditRefusedAdmin,
      "team-hub-edit-not-admin-title",
    ),
  ] {
    let (mut window, _server, (_, pencil)) = on_pencil_until((scenario, state), shown);
    let row = format!("team-hub-row-{pencil}");
    window.until("the row", |window| top(window, &row).is_some());
    let (row, needs) = (
      top(&mut window, &row).expect("its row"),
      top(&mut window, "team-hub-section-needs"),
    );
    let ready = top(&mut window, "team-hub-section-ready").unwrap_or(f32::MAX);
    assert!(
      needs.is_some_and(|needs| needs < row) && row < ready,
      "{scenario}: the row ({row}) is between the Needs you here section ({needs:?}) and the Ready here one ({ready})"
    );
  }
}

#[test]
fn team_qa_hub_edit_the_edit_form_has_the_variables_the_server_needs() {
  let (mut window, _server, _) = on_pencil_until(("team-edit-variables", State::TeamEditVariables), "team-hub-edit");
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  assert!(
    window.texts("team-new-mcp-variables").is_some(),
    "its row is in the form"
  );
  assert!(window.text("team-new-mcp-variables").contains("PENCIL_TOKEN"));
  assert_eq!(
    window.text("team-new-mcp-variables-row-sub"),
    "names only, never values"
  );
}

#[test]
fn team_qa_hub_edit_a_stuck_version_blocks_a_publish_on_top_and_says_so() {
  let (window, server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  server.reject_content("kontur", "");
  let services = window.launch.services().expect("the session");
  let root = window.data.repo("repo-kontur");
  let published = window
    .data
    .runtime
    .block_on(services.servers.publish_mcp_edit(&root, pencil));
  let Err(EditRefused::Failed(why)) = published else {
    panic!("a version on top of a stuck one must not publish: {published:?}");
  };
  assert_eq!(why.key, "sharing.edit.blocked");
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "no second version was made"
  );
}

#[test]
fn team_qa_hub_edit_an_unknown_reason_of_invalid_is_kept_as_plain_invalid() {
  let (window, server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), TEAM_CHIP);
  let root = window.data.repo("repo-kontur");
  let mut fields = pencil_fields(&window, pencil);
  if let Runs::Program { arguments, .. } = &mut fields.runs {
    *arguments = vec!["--odd".to_owned()];
  }
  orchester_workspace::agents::team_mcp_edit::save(&root, pencil, &fields).expect("the draft");
  db(&window)
    .publish_draft::<TeamMcpServer>(pencil, Timestamp::now())
    .expect("published here, waiting");
  server.reject_content("kontur", "something-new");
  let services = window.launch.services().expect("the session");
  window
    .data
    .runtime
    .block_on(services.servers.send_published(&root))
    .expect("sent");
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox")[0].state,
    PublishState::Refused {
      code: "invalid".to_owned()
    },
    "no raw reason of the server is kept"
  );
}

#[test]
fn team_qa_hub_edit_a_send_that_failed_here_after_the_server_may_have_answered_stays_queued() {
  let (window, _server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), TEAM_CHIP);
  let services = window.launch.services().expect("the session");
  let root = window.data.repo("repo-kontur");
  // The server this project is shared on is gone from Settings: the send fails on this computer, not for being offline.
  let mut fields = pencil_fields(&window, pencil);
  if let Runs::Program { arguments, .. } = &mut fields.runs {
    *arguments = vec!["--once".to_owned()];
  }
  orchester_workspace::agents::team_mcp_edit::save(&root, pencil, &fields).expect("the draft");
  let link = db(&window).sharing().expect("read").expect("linked");
  let app = rusqlite::Connection::open(window.data.database()).expect("the application database");
  app
    .execute("DELETE FROM servers WHERE id = ?1", [link.server.to_string()])
    .expect("the server is removed");
  let published = window
    .data
    .runtime
    .block_on(services.servers.publish_mcp_edit(&root, pencil));
  assert!(matches!(published, Err(EditRefused::Failed(_))), "{published:?}");
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "the version stays queued: a retry is idempotent, and the server may hold it"
  );
  assert!(
    draft_of(&window, pencil).is_none(),
    "it was not taken back as the draft"
  );
}

#[test]
fn team_qa_hub_edit_a_send_in_flight_is_not_cancelled_and_is_numbered() {
  let (window, server, (_, pencil)) = on_pencil_until(("team-edit", State::TeamEdit), TEAM_CHIP);
  let services = window.launch.services().expect("the session").clone();
  let root = window.data.repo("repo-kontur");
  let mut fields = pencil_fields(&window, pencil);
  if let Runs::Program { arguments, .. } = &mut fields.runs {
    *arguments = vec!["--slow".to_owned()];
  }
  orchester_workspace::agents::team_mcp_edit::save(&root, pencil, &fields).expect("the draft");
  server.slow_resource_writes(Duration::from_millis(1500));
  let sending = {
    let (servers, root) = (services.servers.clone(), root.clone());
    window
      .data
      .runtime
      .spawn(async move { servers.publish_mcp_edit(&root, pencil).await })
  };
  let deadline = Instant::now() + Duration::from_secs(20);
  let version = loop {
    if let Some(waiting) = db(&window).waiting_publishes().expect("outbox").first() {
      break waiting.version;
    }
    assert!(
      Instant::now() < deadline,
      "the publish never put its version in the outbox"
    );
    std::thread::sleep(Duration::from_millis(10));
  };
  let cancelled = window
    .data
    .runtime
    .block_on(services.servers.reopen_draft(&root, ResourceKind::McpServer, version));
  let Err(why) = cancelled else {
    panic!("a version a send is carrying must not be taken back");
  };
  assert_eq!(why.key, "sharing.errors.beingSent");
  let published = window.data.runtime.block_on(sending).expect("the send ran");
  assert!(matches!(published, Ok(Edited::Published(_))), "{published:?}");
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
  assert_eq!(
    server
      .resources("kontur")
      .iter()
      .filter(|(resource, _)| resource["name"] == "pencil-design")
      .map(|(_, versions)| versions.len())
      .sum::<usize>(),
    2,
    "numbered once, and the app still has it"
  );
}

#[test]
fn team_qa_hub_edit_create_name_taken_state_says_what_was_taken_back_and_why() {
  let (mut window, _server, _) = on_pencil_until(
    ("team-create-name-taken", State::TeamCreateNameTaken),
    "team-hub-taken-back-0-title",
  );
  let title = window.text("team-hub-taken-back-0-title");
  assert!(title.contains("figma-local"), "{title}");
  let body = window.text("team-hub-taken-back-0-body");
  assert!(body.contains("has this name"), "{body}");
  assert!(!body.contains("slug_taken"), "no raw code: {body}");
  assert!(
    db(&window)
      .resources(ResourceKind::McpServer)
      .expect("resources")
      .iter()
      .all(|resource| resource.name().as_str() != "figma-local"),
    "nothing was created"
  );
}

#[test]
fn team_qa_hub_edit_discard_says_why_it_failed() {
  let (mut window, _server, (..)) = on_pencil_until(("team-edit", State::TeamEdit), "team-hub-edit");
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  // The project's database is out of reach for the press, as when the disk fails.
  let file = window.data.repo("repo-kontur").join(".orchester").join("orchester.db");
  let aside = file.with_extension("aside");
  std::fs::rename(&file, &aside).expect("moved aside");
  window.press("team-hub-edit-discard");
  window.until("the reason", |window| {
    window.texts("team-hub-edit-action-error").is_some()
  });
  std::fs::rename(&aside, &file).expect("put back");
}

/// The fields `pencil-design` has as published.
fn pencil_fields(window: &Window, id: ResourceId) -> NewTeamMcpServer {
  let services = window.launch.services().expect("the session");
  let root = window.data.repo("repo-kontur");
  let hub = services
    .workspace
    .team_hub(&root, services.vault.get())
    .expect("the hub reads");
  hub
    .mcp_server(id)
    .and_then(|sheet| sheet.edit.as_ref())
    .expect("an edit")
    .published
    .clone()
}
