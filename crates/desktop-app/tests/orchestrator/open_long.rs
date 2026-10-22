//! Opening a long conversation (ORC-71): 300 turns with long Markdown replies and 50 cards open at the end of the
//! newest turn with only the turns in and near the view built, and settle in a bounded number of frames; so does
//! switching to another long conversation in History. What each opening measured is written under
//! `.tmp/orc-71/profile/` (`support::profile`), with `--features perf_profile` with a lurq profile session's samples.
//! Before the list has measured a turn, lurq builds its first 64 for one pass (`tests/ui_kit/virtualized_list.rs`);
//! the window is read after each frame, once the list has gone to its end.

use lurq::node::element::ElementRef;
use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{
    ActionOutcome, Chat, ChatAction, TurnState,
    change::{Change, ProposedChange},
  },
  resource::ModelId,
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup},
  scroll::{newest_foot_gap, turns},
  support::{
    Window,
    profile::{self, Opening},
  },
};

/// Turns in a long conversation.
const TURNS: usize = 300;
/// Every this many turns, the answer carries a card.
const CARD_EVERY: usize = 6;
/// The most turns built after any frame: those in the view, a turn being taller than it, and lurq's overscan.
const MOST_TURNS: usize = 6;
/// The most frames that change something once the newest turn shows, before nothing changes any more.
const MOST_LAYOUTS: usize = 4;

/// A long reply of the Orchestrator's in Markdown, tagged `tag`: a heading, paragraphs with links and code, a list, a
/// table and a code block.
fn markdown_reply(tag: &str) -> String {
  format!(
    "## {tag}: where the work stands\n\n\
     The review of **KON-12** failed twice on the retry test; the agent fixed the `backoff` helper and the \
     [pipeline run](https://example.invalid/runs/12) is green again. Two tasks wait for a decision.\n\n\
     - KON-14 waits for the person to approve its plan, since the plan widens its file access.\n\
     - KON-15 is queued behind the hold on new starts, which the person set this morning.\n\
     - KON-16 merged; its branch was removed and the base moved twice since.\n\n\
     | Task | State | Next |\n| --- | --- | --- |\n| KON-12 | Running | Review |\n| KON-14 | Waiting | Approve plan |\n\
     | KON-15 | Queued | Release starts |\n\n\
     ```rust\nfn backoff(attempt: u32) -> Duration {{\n  Duration::from_millis(250 * 2u64.pow(attempt.min(6)))\n}}\n```\n\n\
     {tag} closing note: the next check runs when the base moves again, and the summary lands in this conversation."
  )
}

/// The card of the answer of turn `turn`: a release of new starts waiting for the person, or a task the chat created.
fn card(turn: usize) -> ChatAction {
  if turn % (2 * CARD_EVERY) == 0 {
    return ChatAction {
      change: Some(Box::new(ProposedChange {
        subject: String::new(),
        change: Change::Release { task: None },
      })),
      ..ChatAction::new("release_queue", None, ActionOutcome::Waits)
    };
  }
  ChatAction::new("create_task", Some(format!("KON-{turn}")), ActionOutcome::Done)
}

/// A conversation of [`TURNS`] answered turns tagged `tag`, each a short message and a long Markdown reply, every
/// [`CARD_EVERY`]th with a card; it ends `ended` minutes ago.
fn long_chat(screen: &Orchestrator, tag: &str, ended: i64) -> ChatId {
  let mut db = screen.db();
  let model = ModelId::new("haiku").expect("model");
  let chat = ChatId::generate();
  let turns = i64::try_from(TURNS).expect("few turns");
  let start = Timestamp::now().unix_millis() - (turns + ended) * 60_000;
  for turn in 0..TURNS {
    let at = Timestamp::from_unix_millis(start + i64::try_from(turn).expect("few turns") * 60_000);
    let message = ChatMessageId::generate();
    let body = MessageBody::new(&format!("{tag} {turn}: how do the runs stand?")).expect("text");
    db.keep_chat_message(&Chat::start(chat, &body, at), (message, &body), at)
      .expect("kept");
    db.add_chat_reply(chat, (&markdown_reply(&format!("{tag} {turn}")), &model), at)
      .expect("reply");
    if turn % CARD_EVERY == 0 {
      db.add_chat_action(chat, &card(turn), at).expect("card");
    }
    db.end_chat_turn(message, &TurnState::Answered, at).expect("ended");
  }
  chat
}

/// The newest turn's last words in a conversation tagged `tag`.
fn newest(tag: &str) -> String {
  format!("{tag} {} closing note", TURNS - 1)
}

/// The turns `window` built.
fn built(window: &mut Window) -> usize {
  fn walk(element: ElementRef<'_>) -> usize {
    let own = usize::from(element.id().is_some_and(|id| id.starts_with("orchestrator-turn-")));
    own + element.children().into_iter().map(walk).sum::<usize>()
  }
  window.tree.get_element_by_id("orchestrator-scroll").map_or(0, walk)
}

/// Runs the window's frames until conversation `tag` shows its newest turn, then until it settles; what they
/// measured is written as `name` (`support::profile::write`), with lurq's `perf_profile` with a profile session's
/// samples.
fn opening(screen: &mut Orchestrator, name: &str, tag: &str) -> Opening {
  #[cfg(feature = "perf_profile")]
  let session = profile::session::Session::start(&screen.window);
  let last = newest(tag);
  let opened = profile::open(&mut screen.window, built, |window| {
    window
      .texts("orchestrator-conversation")
      .is_some_and(|texts| texts.join(" ").contains(&last))
  });
  #[cfg(feature = "perf_profile")]
  let samples = session.end();
  #[cfg(not(feature = "perf_profile"))]
  let samples = String::new();
  profile::write(name, &opened, &samples);
  opened
}

/// Asserts `opened` had at most [`MOST_TURNS`] turns built after any frame, settled within [`MOST_LAYOUTS`] frames
/// that changed something, and ended at its newest turn's end.
fn bounded(screen: &mut Orchestrator, opened: &Opening) {
  assert!(
    opened.most_built() <= MOST_TURNS,
    "at most {MOST_TURNS} of {TURNS} turns are built: {opened:?}"
  );
  assert!(
    opened.layouts_to_settle() <= MOST_LAYOUTS,
    "it settles within {MOST_LAYOUTS} frames: {opened:?}"
  );
  let gap = newest_foot_gap(screen);
  assert!(gap.abs() < 1.0, "it opens at its newest turn's end: {gap} under it");
  assert!(!turns(screen).is_empty(), "the newest turns are built");
}

#[test]
fn orchestrator_open_long_conversation_builds_few_turns_and_settles() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = long_chat(&screen, "Long", 0);
  screen.settings();
  screen.window.navigation().go(Route::Project(
    screen.project,
    ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)),
  ));
  let opened = opening(&mut screen, "open-long", "Long");
  bounded(&mut screen, &opened);
}

#[test]
fn orchestrator_open_long_switching_in_history_builds_few_turns_and_settles() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let older = long_chat(&screen, "Older", 600);
  let newer = long_chat(&screen, "Newer", 0);
  screen.go(OrchestratorChat::Chat(newer));
  opening(&mut screen, "switch-first", "Newer");
  screen.click("orchestrator-history");
  let row = format!("orchestrator-history-{older}");
  screen.until("History", |screen| screen.exists(&row));
  screen
    .window
    .tree
    .get_element_by_id_mut(&row)
    .expect("the older conversation is listed")
    .click();
  let opened = opening(&mut screen, "switch-long", "Older");
  bounded(&mut screen, &opened);
}
