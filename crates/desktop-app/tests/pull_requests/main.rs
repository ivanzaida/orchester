//! Pull requests in the app's real window on the QA seed's projects (`examples/seed`, the `pull-requests`
//! scenario, whose GitHub connection runs a fake `gh` that answers from files): the list by tab with each
//! pull request's state, review, checks and task, searched and opened (`list`), and what the page shows
//! when it cannot list them: no linked repository, the first sync reading, an empty tab, and each way a sync
//! fails (`states`). Needs `git`, as the seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod detail;
mod feedback;
mod harness;
mod list;
mod states;
