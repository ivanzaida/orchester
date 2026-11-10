//! What the runs-on picker says under its efforts or in its tooltips only in a state of the project's: a draft saved in
//! the Orchestrator's settings that choosing replaces (`Hxq6v` in `Runs-on picker · states` `t7cdi`), and that choosing
//! a connection the conversation's session does not live on starts a new session, even when it is no sign-in
//! (`Another account` `vsqci`).

use orchester_core::{orchestrator::OrchestratorSettings, resource::ModelId, store::ConnectionStore, time::Timestamp};

use super::{
  connections,
  harness::{Orchestrator, Setup},
  history::kept,
  runs_on::{MENU, on_account, on_rail, open, part, published, rest_on},
};

#[test]
fn orchestrator_picker_says_that_choosing_replaces_the_draft_saved_in_the_settings() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (_, settings) = published(&screen).expect("published");
  let draft = OrchestratorSettings::new(
    settings.connection(),
    ModelId::new("opus").expect("model"),
    Some("low".to_owned()),
  );
  screen
    .db()
    .save_orchestrator_draft(&draft, Timestamp::now())
    .expect("a draft");
  open(&mut screen);
  screen.until("the note", |screen| {
    screen.exists(&part("note-0"))
      && screen.text(&part("note-0"))
        == "Choosing here publishes a new version and replaces the draft saved in Orchestrator settings."
  });
}

#[test]
fn orchestrator_picker_says_a_connection_that_is_no_sign_in_starts_a_new_session_too() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (work, at_work) = on_account(&screen, "Work", true);
  let chat = kept(&screen, "What runs?", Timestamp::now(), "Nothing yet.");
  screen
    .db()
    .set_chat_session(chat, "session-1", (at_work, work.id()))
    .expect("session");
  let program = std::env::current_exe().expect("a program");
  let deepseek = connections::api_provider("DeepSeek", &program);
  screen
    .db()
    .create_connection(&deepseek)
    .expect("the API provider's connection");
  screen
    .db()
    .publish_orchestrator(
      &OrchestratorSettings::new(at_work, ModelId::new("haiku").expect("model"), None),
      Timestamp::now(),
    )
    .expect("published");
  screen.go(orchester_app::routes::OrchestratorChat::Chat(chat));
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Nothing yet.")
  });
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists(MENU));
  rest_on(&mut screen, &on_rail(deepseek.id()));
  assert_eq!(
    screen.text(&format!("{}-tip-tooltip", on_rail(deepseek.id()))),
    "DeepSeek · api.deepseek.com Another account starts a new session: earlier messages aren't remembered.",
    "a switch to an API provider starts one"
  );
  rest_on(&mut screen, &on_rail(at_work));
  assert_eq!(
    screen.text(&format!("{}-tip-tooltip", on_rail(at_work))),
    "Claude Code · Work",
    "the session's own connection and account start nothing new"
  );
}
