//! The slash menu in the Orchestrator's composer (ORC-321, `07` `jAiyu`, `07b` `m85qp`, `07d` `R8eum`): `/` at the
//! start of the draft opens the chat's own commands, typing filters them, ↑ ↓ and Enter or a click run one as its
//! control does (*New conversation* as the header's *New*, *History* as its toggle), and the typed `/` goes; a command
//! that cannot run is greyed with why; Esc closes the menu and keeps the text; a `/` inside text is just text. Compact
//! and Model and effort are not offered until their controls exist. The project's published skills follow the commands
//! (ORC-325), a chosen one becomes the composer's chip, and it goes with the message as the CLI's own (ORC-634 on
//! Claude Code, ORC-663 on Codex).

use std::{
  fs,
  time::{Duration, Instant},
};

use orchester_app::{
  routes::{OrchestratorChat, ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{
  orchestrator::EntryKind,
  resource::{InstructionScope, Skill},
  time::Timestamp,
};
use orchester_workspace::skills::editor;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

const INPUT: &str = "orchestrator-input";
const MENU: &str = "orchestrator-composer-slash-menu";
const NEW: &str = "orchestrator-composer-slash-new-conversation";
const HISTORY: &str = "orchestrator-composer-slash-history";
const NOTES: &str = "orchestrator-composer-slash-skill-release-notes";
const CHIP: &str = "orchestrator-composer-skill-chip";
const HELD: &str = "orchestrator-composer-skill-held";

/// The field with the focus, holding `text` with the caret at its end.
fn write(screen: &mut Orchestrator, text: &str) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, text);
}

fn draft(screen: &mut Orchestrator) -> String {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .and_then(|element| element.as_text_input())
    .expect("the message field")
    .value()
}

/// The Orchestrator on a kept conversation.
fn on_a_conversation() -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  kept(&screen, "Order of the auth work", Timestamp::now(), "Auth first.");
  screen.go(OrchestratorChat::Latest);
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Auth first.")
  });
  screen
}

#[test]
fn orchestrator_slash_lists_the_chats_commands_and_runs_history_by_keyboard() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/");
  screen.until("the slash menu", |screen| {
    screen.exists(MENU) && screen.exists(NEW) && screen.exists(HISTORY)
  });
  let shown = screen.text(MENU);
  assert!(shown.starts_with("Commands"), "{shown}");
  assert!(shown.contains("Type to filter"), "{shown}");
  for missing in ["Compact", "Model and effort", "Permissions", "Stop turn"] {
    assert!(
      !shown.contains(missing),
      "no {missing} until its control exists: {shown}"
    );
  }
  assert!(screen.text(NEW).contains("Enter"), "the focus starts on the first row");
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.until("the focus on History", |screen| {
    screen.text(HISTORY).contains("Enter") && !screen.text(NEW).contains("Enter")
  });
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.until("the focus back on the first row", |screen| {
    screen.text(NEW).contains("Enter")
  });

  write(&mut screen, "/hist");
  screen.until("only History to match", |screen| {
    screen.exists(HISTORY) && !screen.exists(NEW)
  });
  let shown = screen.text(MENU);
  assert!(shown.contains("1 of 6"), "{shown}");
  assert!(shown.contains("/hist"), "{shown}");
  screen.key(("Enter", "Enter"), false);
  screen.until("History open", |screen| {
    screen.exists("orchestrator-history-panel") && !screen.exists(MENU)
  });
  assert_eq!(draft(&mut screen), "", "the typed / goes");
}

#[test]
fn orchestrator_slash_new_conversation_is_greyed_on_a_new_one_and_runs_on_a_kept_one() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/new");
  screen.until("New conversation", |screen| screen.exists(NEW));
  screen.click(NEW);
  screen.until("a new conversation", |screen| {
    screen.route() == Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::New))
  });
  assert!(!screen.exists(MENU), "the menu closes");

  write(&mut screen, "/");
  screen.until("the slash menu", |screen| screen.exists(NEW));
  let row = screen.text(NEW);
  assert!(row.contains("This is a new conversation already"), "why: {row}");
  assert!(!row.contains("Enter"), "it cannot take the focus: {row}");
  assert!(
    screen.text(HISTORY).contains("Enter"),
    "the focus starts on the first row that can run"
  );
  screen.click(NEW);
  assert!(screen.exists(MENU), "a greyed command does nothing");
  assert_eq!(draft(&mut screen), "/");
}

#[test]
fn orchestrator_slash_is_text_elsewhere_and_esc_keeps_it() {
  let mut screen = on_a_conversation();
  write(&mut screen, "Ask /new");
  screen.window.pass();
  assert!(!screen.exists(MENU), "a / inside text is text");
  write(&mut screen, "/new conversation");
  screen.window.pass();
  assert!(!screen.exists(MENU), "a space ends the command");

  write(&mut screen, "/hi");
  screen.until("the slash menu", |screen| screen.exists(MENU));
  screen.key(("Escape", "Escape"), false);
  screen.until("the menu to close", |screen| !screen.exists(MENU));
  assert_eq!(draft(&mut screen), "/hi", "Esc keeps the text as typed");
  assert!(!screen.exists("orchestrator-history-panel"), "nothing ran");

  write(&mut screen, "/hin");
  screen.window.pass();
  assert!(!screen.exists(MENU), "it stays closed while that / is typed on");
  // Once the draft no longer starts with `/`, a new `/` opens it again.
  write(&mut screen, "");
  write(&mut screen, "/deploy");
  screen.until("nothing to match", |screen| {
    screen.text(MENU).contains("No command or skill matches /deploy")
  });
  let shown = screen.text(MENU);
  assert!(shown.contains("Send as text"), "{shown}");
  screen.key(("Enter", "Enter"), false);
  screen.until("the text sent as typed", |screen| {
    screen.text("orchestrator-conversation").contains("Noted: /deploy")
  });
}

/// The menu opens over the composer's box (`jAiyu`): 640 wide, its left edge at the box's, its bottom 8 over it.
#[test]
fn orchestrator_slash_menu_opens_over_the_box() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/");
  screen.until("the slash menu", |screen| screen.exists(MENU));
  screen.window.pass();
  let menu = screen.bounds(MENU);
  let boxed = screen.bounds("orchestrator-composer");
  assert!((menu.width - 640.0).abs() < 0.5, "{menu:?}");
  assert!((menu.x - boxed.x).abs() < 0.5, "{menu:?} {boxed:?}");
  assert!(
    (boxed.y - (menu.y + menu.height) - 8.0).abs() < 0.5,
    "{menu:?} {boxed:?}"
  );
}

/// The watch commands after `/wa` (ORC-771, `01 · Watch commands` `h1KHAx`): *Watch modules*, *Watch tasks*, *Stop
/// watching* and *What I watch*, the typed letters in bold. Until their pickers exist the first two are faded with
/// why; the last two are faded while the conversation watches nothing. None takes the focus or a press.
#[test]
fn orchestrator_slash_offers_the_watch_commands_faded_while_nothing_is_watched() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/wa");
  let row = |name: &str| format!("orchestrator-composer-slash-{name}");
  screen.until("the watch commands", |screen| screen.exists(&row("watching")));
  let shown = screen.text(MENU);
  assert!(shown.contains("4 of 6"), "{shown}");
  assert!(!screen.exists(NEW) && !screen.exists(HISTORY), "{shown}");
  let rows = [
    ("watch-module", "Watch modules", "Not available in this version yet"),
    ("watch-task", "Watch tasks", "Not available in this version yet"),
    ("unwatch", "Stop watching", "Watches nothing yet"),
    ("watching", "What I watch", "Watches nothing yet"),
  ];
  let mut tops = Vec::new();
  for (name, label, why) in rows {
    let words = screen.text(&row(name)).replace('\u{a0}', " ");
    assert!(words.contains(why), "{name}: {words}");
    assert!(
      words.replace(' ', "").contains(&label.replace(' ', "")),
      "{name}: {words}"
    );
    assert!(!words.contains("Enter"), "{name} cannot take the focus: {words}");
    tops.push(screen.bounds(&row(name)).y);
  }
  assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "in 01's order: {tops:?}");
  screen.click(&row("unwatch"));
  assert!(screen.exists(MENU), "a faded command does nothing");
  assert_eq!(draft(&mut screen), "/wa");
}

/// The project's skills (ORC-325): `release-notes` published, `draft-only` never published.
fn publish_skills(screen: &Orchestrator) {
  let skill = |description: &str| {
    Skill::new(description.into(), "Body".into(), Vec::new(), InstructionScope::Project).expect("a skill")
  };
  let notes = editor::save_in(
    &screen.root,
    None,
    "release-notes",
    &skill("Release notes from the merged tasks"),
  )
  .expect("a draft");
  editor::publish_in(&screen.root, &notes).expect("v1");
  editor::save_in(&screen.root, None, "draft-only", &skill("Never published")).expect("a draft");
}

/// The Skills section (`IQOLN`, `OKWFL`) lists the published skills, never a draft; choosing one puts its chip before
/// the message (`07c` `f0b717`), whose `x` removes it.
#[test]
fn orchestrator_slash_offers_published_skills_and_a_chosen_one_becomes_a_chip() {
  let mut screen = on_a_conversation();
  publish_skills(&screen);
  write(&mut screen, "/");
  screen.until("the skills", |screen| screen.exists(NOTES));
  let shown = screen.text(MENU);
  assert!(shown.contains("Skills") && shown.contains("1 published"), "{shown}");
  assert!(shown.contains("Release notes from the merged tasks"), "{shown}");
  assert!(!shown.contains("draft-only"), "a draft never shows: {shown}");

  write(&mut screen, "/rel");
  screen.until("only the skill to match", |screen| {
    screen.exists(NOTES) && !screen.exists(NEW) && !screen.exists(HISTORY)
  });
  assert!(screen.text(MENU).contains("1 of 1"), "{}", screen.text(MENU));
  assert!(screen.text(NOTES).contains("Enter"), "the focus is on the skill");
  screen.key(("Enter", "Enter"), false);
  screen.until("the chip", |screen| screen.exists(CHIP) && !screen.exists(MENU));
  assert_eq!(draft(&mut screen), "", "the typed / goes");
  assert!(screen.text(CHIP).contains("release-notes"), "{}", screen.text(CHIP));
  assert!(!screen.exists(HELD), "a skill holds no message back");

  screen.click(&format!("{CHIP}-remove"));
  screen.until("the chip to go", |screen| !screen.exists(CHIP));
  assert_eq!(draft(&mut screen), "", "the draft stays as it was");
}

/// ORC-634: a message with a chosen skill is kept with the skill's block before the words, and goes to the
/// Orchestrator on Claude Code as the CLI's own skill: the runner starts it with the skill's plugin and slash commands
/// on, but still no setting source, and gives it the plugin's command with the words after it (the fake CLI answers
/// `Noted: <prompt>`). The conversation shows the skill's chip over the person's words, never the block, and the
/// composer's chip empties once the message is kept.
#[test]
fn orchestrator_sends_a_chosen_skill_as_claude_codes_own_and_shows_its_chip_on_the_message() {
  let mut screen = on_a_conversation();
  publish_skills(&screen);
  write(&mut screen, "/rel");
  screen.until("the skill", |screen| screen.exists(NOTES));
  screen.key(("Enter", "Enter"), false);
  screen.until("the chip", |screen| screen.exists(CHIP));

  write(&mut screen, "for v0.4");
  screen.key(("Enter", "Enter"), true);
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: /orchester:release-notes for v0.4")
  });
  screen.until("the chip to go", |screen| !screen.exists(CHIP));
  let started = fs::read_to_string(screen.root.join(".orchester").join("fake-chat.log")).expect("the agent's log");
  let argv = started
    .lines()
    .rfind(|line| line.starts_with("argv: "))
    .expect("how the turn started");
  assert!(argv.contains("--plugin-dir "), "{argv}");
  assert!(argv.contains(".orchester") && argv.contains("chat-skills"), "{argv}");
  assert!(!argv.contains("--disable-slash-commands"), "{argv}");
  assert!(argv.contains("--setting-sources  "), "no setting source: {argv}");

  let db = screen.db();
  let chat = db.chats().expect("chats")[0].chat.id();
  let message = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Person { message, text, .. } if text.as_str().ends_with("for v0.4") => Some((message, text)),
      _ => None,
    })
    .expect("the message kept");
  assert!(
    message
      .1
      .as_str()
      .starts_with("<orchester-skill name=\"release-notes\" version=\"1\""),
    "{}",
    message.1.as_str()
  );
  let sent = format!("orchestrator-message-{}-skill-chip", message.0);
  screen.until("the sent message's chip", |screen| screen.exists(&sent));
  assert!(screen.text(&sent).contains("release-notes"));
  let shown = screen.text("orchestrator-conversation");
  assert!(!shown.contains("orchester-skill"), "the block never shows: {shown}");
}

/// ORC-663: on Codex the chosen skill is Codex's own too: the chip's tooltip says so, and the runner registers the
/// skill's folder for the turn's App Server and gives it `$<slug>` with the words after it and the skill named (the
/// fake answers `Noted: <prompt>` and logs whether the skill it was given is one it registered).
#[test]
fn orchestrator_sends_a_chosen_skill_as_codexs_own_and_its_chip_says_so() {
  let mut screen = Orchestrator::new(Setup::Codex);
  screen.go(OrchestratorChat::New);
  publish_skills(&screen);
  write(&mut screen, "/rel");
  screen.until("the skill", |screen| screen.exists(NOTES));
  screen.key(("Enter", "Enter"), false);
  screen.until("the chip", |screen| screen.exists(CHIP));
  screen.window.hover(CHIP);
  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  let tooltip = format!("{CHIP}-tooltip");
  screen.until("the chip's tooltip", |screen| screen.exists(&tooltip));
  let said = screen.text(&tooltip);
  assert!(
    said.contains("release-notes · Published v1") && said.contains("Codex loads it as its own skill ($release-notes)"),
    "{said}"
  );
  // Over the chip, its left edge at the chip's, 6 clear of the composer's top edge, its words wrapping at 360
  // (`mhanX` in `07` `QnPRC`).
  screen.window.pass();
  let (tip, chip, boxed) = (
    screen.bounds(&tooltip),
    screen.bounds(CHIP),
    screen.bounds("orchestrator-composer"),
  );
  assert!((tip.x - chip.x).abs() < 0.5, "{tip:?} {chip:?}");
  assert!((boxed.y - (tip.y + tip.height) - 6.0).abs() < 0.5, "{tip:?} {boxed:?}");
  assert!(tip.width <= 360.0 + 16.0 + 0.5, "it wraps at 360: {tip:?}");
  assert!(tip.height > 30.0, "its words wrap: {tip:?}");

  write(&mut screen, "for v0.4");
  screen.key(("Enter", "Enter"), true);
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: $release-notes for v0.4")
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let log = fs::read_to_string(screen.root.join(".orchester").join("fake-chat.log")).expect("the agent's log");
  assert!(log.contains("skill: release-notes loaded"), "{log}");
  let shown = screen.text("orchestrator-conversation");
  assert!(!shown.contains("orchester-skill"), "the block never shows: {shown}");
}
