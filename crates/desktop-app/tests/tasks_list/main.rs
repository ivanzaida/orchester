//! The Tasks list on the QA seed's projects (`examples/seed`), in the app's real window: its workflow tabs
//! and their counts, the search, the pipeline filter and the sorts (`query`), what each row says and where
//! it leads (`rows`), and what the list shows instead of rows (`states`). No runner serves the project, so
//! a run recorded as running reads as interrupted. Needs `git`, as the seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod harness;
mod query;
mod rows;
mod states;
