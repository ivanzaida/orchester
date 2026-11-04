//! What a person does with a conversation in History (ORC-328, `08` `q30JYF`, `08b` `sLeGr`, `08c` `r4DVU`): the `⋯`
//! button under the pointer and a right-click open the row's menu, *Rename* turns the title into a field where Enter
//! keeps it, a blank one is refused and Esc cancels, and *Pin* lists the conversation under *Pinned* at the top until
//! *Unpin*. Run with `cargo test -p orchester-app --test orchestrator history_rows`.

use lurq::app::events::MouseButton;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{id::ChatId, time::Timestamp};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

/// History open over the latest of two conversations: the older one's id and the newer one's.
pub(super) fn opened() -> (Orchestrator, ChatId, ChatId) {
  let mut screen = Orchestrator::new(Setup::Ready);
  let week_ago = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 7 * 24 * 3_600_000);
  let older = kept(&screen, "Why did KON-11 fail?", week_ago, "Its token expired.");
  let newer = kept(&screen, "Plan the GitHub app permissions", Timestamp::now(), "Done.");
  screen.go(OrchestratorChat::Latest);
  screen.until("the latest conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Done.")
  });
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  (screen, older, newer)
}

/// Moves the pointer over the middle of `id`.
pub(super) fn hover(screen: &mut Orchestrator, id: &str) -> (f32, f32) {
  let frame = screen.bounds(id);
  let (x, y) = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
  screen.window.tree.mouse_move(x, y);
  screen.window.pass();
  (x, y)
}

/// Types `title` into the open title field of `chat` and presses Enter there.
fn enter_title(screen: &mut Orchestrator, chat: ChatId, title: &str) {
  let field = format!("orchestrator-history-rename-field-{chat}");
  screen.set(&field, title);
  screen
    .window
    .tree
    .get_element_by_id_mut(&field)
    .expect("the title field")
    .focus();
  screen.key(("Enter", "Enter"), false);
}

#[test]
fn history_rows_rename_in_place_keeps_the_title_refuses_a_blank_one_and_esc_cancels() {
  let (mut screen, older, _) = opened();
  let row = format!("orchestrator-history-{older}");
  assert!(
    !screen.exists(&format!("orchestrator-history-more-{older}")),
    "the ⋯ button shows only under the pointer"
  );
  hover(&mut screen, &row);
  screen.click(&format!("orchestrator-history-more-{older}"));
  let menu = screen.text(&format!("orchestrator-history-menu-{older}"));
  assert_eq!(menu, "Rename Pin Archive Delete…", "only what works: {menu}");
  screen.click(&format!("orchestrator-history-rename-{older}"));
  let field = format!("orchestrator-history-rename-field-{older}");
  screen.until("the title field", |screen| screen.exists(&field));
  assert!(
    screen
      .text(&format!("orchestrator-history-rename-keys-{older}"))
      .contains("Enter saves")
  );
  enter_title(&mut screen, older, "   ");
  screen.until("the blank title refused", |screen| {
    screen.exists(&format!("{field}-note"))
  });
  assert!(screen.exists(&field), "the field stays open");
  assert_eq!(
    screen.db().chat(older).expect("read").expect("kept").title(),
    "Why did KON-11 fail?"
  );
  enter_title(&mut screen, older, "KON-11 failure: expired GitHub token");
  screen.until("the title kept", |screen| !screen.exists(&field));
  screen.until("the row titled anew", |screen| {
    screen.text(&row).starts_with("KON-11 failure: expired GitHub token")
  });
  assert_eq!(
    screen.db().chat(older).expect("read").expect("kept").title(),
    "KON-11 failure: expired GitHub token",
    "kept in the project's data, as it is read after a restart"
  );
  hover(&mut screen, &row);
  screen.key(("F2", "F2"), false);
  screen.until("the title field again", |screen| screen.exists(&field));
  screen.set(&field, "Something else");
  screen.key(("Escape", "Escape"), false);
  screen.until("the edit canceled", |screen| !screen.exists(&field));
  assert!(
    screen.exists("orchestrator-history-panel"),
    "Esc cancels the edit before it closes History"
  );
  assert!(screen.text(&row).starts_with("KON-11 failure: expired GitHub token"));
}

#[test]
fn history_rows_pin_lists_the_conversation_under_pinned_at_the_top_until_unpinned() {
  let (mut screen, older, newer) = opened();
  assert!(!screen.exists("orchestrator-history-heading-Pinned"));
  let (x, y) = hover(&mut screen, &format!("orchestrator-history-{older}"));
  screen.window.tree.mouse_down(x, y, MouseButton::Right);
  screen.window.tree.mouse_up(x, y, MouseButton::Right);
  screen.window.pass();
  screen.until("the menu at the pointer", |screen| {
    screen.exists(&format!("orchestrator-history-menu-{older}"))
  });
  screen.click(&format!("orchestrator-history-pin-{older}"));
  screen.until("the Pinned group", |screen| {
    screen.exists("orchestrator-history-heading-Pinned")
  });
  let pinned = screen.bounds(&format!("orchestrator-history-{older}"));
  let today = screen.bounds(&format!("orchestrator-history-{newer}"));
  assert!(pinned.y < today.y, "the pinned conversation is listed first");
  let listing = screen.db().chats().expect("History");
  assert!(
    listing
      .iter()
      .any(|listing| listing.chat.id() == older && listing.pinned.is_some()),
    "kept in the project's data"
  );
  hover(&mut screen, &format!("orchestrator-history-{older}"));
  screen.click(&format!("orchestrator-history-more-{older}"));
  assert!(
    screen
      .text(&format!("orchestrator-history-menu-{older}"))
      .contains("Unpin")
  );
  screen.click(&format!("orchestrator-history-pin-{older}"));
  screen.until("the Pinned group gone", |screen| {
    !screen.exists("orchestrator-history-heading-Pinned")
  });
  assert!(
    screen
      .db()
      .chats()
      .expect("History")
      .iter()
      .all(|listing| listing.pinned.is_none())
  );
}
