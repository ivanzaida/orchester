//! The slash menu in a task Conversation's composer (ORC-321, `06` `yKWQx`, `06b` `W4SNj3`): `/` at the start of the
//! draft opens the run's own commands; *Stop turn* is greyed with why while no turn of an agent runs, and Enter then
//! neither runs it nor sends; Esc closes the menu and keeps the text; a `/` inside text is just text. Compact, Model
//! and effort, and Permissions are not offered until their controls exist. That *Stop turn* stops a running turn is
//! `composer_stop_turn`'s, on a live run. A chosen skill goes with the message in its block (ORC-634).

use orchester_app::ui::slash::{matched, query};
use orchester_core::resource::{InstructionScope, Skill};
use orchester_workspace::skills::editor;

use super::{click, draft, key, messages, open, seeded, text, type_on, write};
use crate::sending::ENTER;

#[test]
fn composer_slash_opens_only_for_a_leading_slash_and_matches_letters_ignoring_case() {
  assert_eq!(query("/"), Some(""));
  assert_eq!(query("/co"), Some("co"));
  assert_eq!(query("Ask /co"), None, "a / inside text");
  assert_eq!(query("/co now"), None, "a space ends it");
  assert_eq!(query("/co\n"), None, "a new line ends it");
  assert_eq!(query(""), None);
  assert_eq!(matched("New conversation", ""), Some(0..0));
  assert_eq!(matched("New conversation", "co"), Some(4..6));
  assert_eq!(matched("Compact", "co"), Some(0..2), "ignoring case");
  assert_eq!(matched("Stop turn", "ST"), Some(0..2));
  assert_eq!(matched("History", "new"), None);
  assert_eq!(matched("Ärger", "är"), Some(0..3), "by letters, not bytes");
}

const MENU: &str = "composer-slash-menu";
const STOP: &str = "composer-slash-stop-turn";
const TITLE: &str = "Localize the invoice emails";

#[test]
fn composer_slash_offers_stop_turn_greyed_with_why_while_no_turn_runs() {
  let (mut window, seeded) = seeded();
  let task = open(&mut window, &seeded, TITLE);
  write(&mut window, "/");
  window.until("the slash menu", |window| {
    window.tree.get_element_by_id(MENU).is_some() && window.tree.get_element_by_id(STOP).is_some()
  });
  let shown = text(&mut window, MENU);
  assert!(shown.starts_with("Commands"), "{shown}");
  for missing in [
    "Compact",
    "Model and effort",
    "Permissions",
    "New conversation",
    "History",
  ] {
    assert!(!shown.contains(missing), "no {missing} here: {shown}");
  }
  let row = text(&mut window, STOP);
  assert!(row.contains("Stop turn"), "{row}");
  assert!(row.contains("No turn is running"), "why: {row}");
  assert!(!row.contains("Enter"), "a greyed command takes no focus: {row}");

  key(&mut window, ENTER, (false, false));
  assert!(
    window.tree.get_element_by_id(MENU).is_some(),
    "Enter runs nothing greyed"
  );
  assert_eq!(draft(&mut window), "/");
  assert!(messages(&seeded, &task).is_empty(), "nor sends the /");

  type_on(&mut window, "st");
  window.until("the filter", |window| text(window, MENU).contains("1 of 1"));
  key(&mut window, ("Escape", "Escape"), (false, false));
  window.until("the menu to close", |window| {
    window.tree.get_element_by_id(MENU).is_none()
  });
  assert_eq!(draft(&mut window), "/st", "Esc keeps the text as typed");
}

#[test]
fn composer_slash_inside_text_is_text() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, TITLE);
  write(&mut window, "Ask /stop");
  window.pass();
  assert!(window.tree.get_element_by_id(MENU).is_none(), "a / inside text is text");
  assert_eq!(draft(&mut window), "Ask /stop");
}

const SKILL: &str = "composer-slash-skill-rust-change";
const CHIP: &str = "composer-skill-chip";
const HELD: &str = "composer-skill-held";

/// The project's published skill `rust-change` (ORC-325), and `draft-only`, never published.
fn publish_skills(repo: &std::path::Path) {
  let skill = |description: &str| {
    Skill::new(description.into(), "Body".into(), Vec::new(), InstructionScope::Project).expect("a skill")
  };
  let rust = editor::save_in(
    repo,
    None,
    "rust-change",
    &skill("Checks to run when a change touches Rust"),
  )
  .expect("a draft");
  editor::publish_in(repo, &rust).expect("v1");
  editor::save_in(repo, None, "draft-only", &skill("Never published")).expect("a draft");
}

/// The Skills section follows the run's commands (`06` `yKWQx`): the published skills, never a draft; with *Stop
/// turn* greyed the focus starts on the skill, and Enter puts its chip before the message (`06c` `DunaJ`); its `x`
/// removes it, and the words stay.
#[test]
fn composer_slash_offers_published_skills_and_a_chosen_one_becomes_a_chip() {
  let (mut window, seeded) = seeded();
  publish_skills(&seeded.repo);
  open(&mut window, &seeded, TITLE);
  write(&mut window, "/");
  window.until("the skills", |window| window.tree.get_element_by_id(SKILL).is_some());
  let shown = text(&mut window, MENU);
  assert!(shown.contains("Skills") && shown.contains("1 published"), "{shown}");
  assert!(!shown.contains("draft-only"), "a draft never shows: {shown}");
  assert!(
    text(&mut window, SKILL).contains("Enter"),
    "the focus skips the greyed command"
  );

  key(&mut window, ENTER, (false, false));
  window.until("the chip", |window| {
    window.tree.get_element_by_id(CHIP).is_some() && window.tree.get_element_by_id(MENU).is_none()
  });
  assert!(!draft(&mut window).starts_with('/'), "the typed / goes");
  assert!(text(&mut window, CHIP).contains("rust-change"));
  assert!(
    window.tree.get_element_by_id(HELD).is_none(),
    "a message is not held back"
  );

  type_on(&mut window, "skip");
  click(&mut window, &format!("{CHIP}-remove"));
  window.until("the chip to go", |window| window.tree.get_element_by_id(CHIP).is_none());
  assert_eq!(draft(&mut window), "skip", "the words stay");
}

/// ORC-634: a message with a chosen skill is kept with the skill's block before the words, as every agent of the run
/// reads it, whatever its engine; the composer's chip empties, and the conversation shows the skill's chip over the
/// person's words, never the block.
#[test]
fn composer_sends_a_chosen_skill_in_its_block_and_shows_its_chip_on_the_message() {
  let (mut window, seeded) = seeded();
  publish_skills(&seeded.repo);
  let task = open(&mut window, &seeded, TITLE);
  write(&mut window, "/rust");
  window.until("the skill", |window| window.tree.get_element_by_id(SKILL).is_some());
  key(&mut window, ENTER, (false, false));
  window.until("the chip", |window| window.tree.get_element_by_id(CHIP).is_some());
  type_on(&mut window, "skip");
  key(&mut window, ENTER, (false, false));
  window.until("the message stored", |window| {
    window.tree.get_element_by_id(CHIP).is_none() && !messages(&seeded, &task).is_empty()
  });
  let stored = messages(&seeded, &task).remove(0);
  let body = stored.body().as_str();
  assert!(
    body.starts_with("<orchester-skill name=\"rust-change\" version=\"1\""),
    "{body}"
  );
  assert!(body.contains("Checks to run when a change touches Rust"), "{body}");
  assert!(body.ends_with("</orchester-skill>\n\nskip"), "{body}");
  assert!(!draft(&mut window).contains("skip"), "the draft went with it");

  let sent = format!("conversation-message-{}-skill-chip", stored.id());
  window.until("the sent message's chip", |window| {
    window.tree.get_element_by_id(&sent).is_some()
  });
  assert!(text(&mut window, &sent).contains("rust-change"));
  let shown = text(&mut window, &format!("conversation-message-{}", stored.id()));
  assert!(shown.contains("skip") && !shown.contains("orchester-skill"), "{shown}");
}
