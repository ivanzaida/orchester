//! The Tasks list on the QA seed's projects (`examples/seed`), in the app's real window: its workflow tabs
//! and their counts, the search, the pipeline filter and the sorts (`query`), what each row says and where
//! it leads (`rows`), what the list shows instead of rows (`states`), what waits outside the tasks on
//! Needs you (`needs_you`), and closing and deleting a task from a row or its page (`close`). No runner serves the
//! project, so a run recorded as running reads as interrupted. Needs `git`, as the seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod close;
mod harness;
mod needs_you;
mod query;
mod rows;
mod states;
