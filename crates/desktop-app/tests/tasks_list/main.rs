//! The Tasks list on the QA seed's projects (`examples/seed`), in the app's real window (ORC-99): its views and their
//! counts, the search, the Filter with its chips and the sorts in Display (`query`), Active's groups and what waits
//! outside the tasks (`needs_you`), what each row says, what the pointer brings up on it and where it leads (`rows`),
//! what the list shows instead of rows, its skeletons among them (`states`), closing and deleting a task from a row or
//! its page (`close`), and the board beside the list (`board`, ORC-105), on which nothing moves while a card is lifted
//! or a question is open (`board_layout`), and a shared project's tasks coming down after a join (`download`,
//! ORC-282, against the workspace tests' loopback `responder`). No runner serves the project, so a run recorded as
//! running reads as interrupted. Needs `git`, as the seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod board;
mod board_layout;
mod close;
mod download;
mod harness;
mod needs_you;
mod query;
#[path = "../../../workspace/tests/shared_download/responder.rs"]
#[allow(
  dead_code,
  reason = "the workspace's first-download tests share the responder; these use a part"
)]
mod responder;
mod rows;
mod states;
