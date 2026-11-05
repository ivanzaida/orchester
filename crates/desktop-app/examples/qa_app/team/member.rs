//! The member, role and outage states (ORC-716, ORC-717) and the claim, module and Sharing states (ORC-732): Kontur's
//! tasks and modules as the team server reads them, with the person a member (or an Admin, or the server's admin) and
//! the Admins named as `Row 1 · Member views` (`rD8mt`) names them; the modules Webhooks v2 (held by olha), Billing
//! exports (held by the person) and API rate limits (held by nobody) as 01d `rvXxg` draws them, and KON-12 claimed by
//! the person and assigned to them; what each ORC-732 state changes of them is in [`super::work`]. The tasks this
//! computer runs keep their ids here, so their runs join the server's tasks. In the outage the server's reads are
//! unavailable, from the start, once the tasks were shown, or once a module's page read its module. Fixture data for
//! QA, never shipped.

mod modules;

use std::{collections::HashMap, time::Duration};

use orchester_api::test_support::team_server::{AuthorityClaim, AuthorityTask, Person, Standing, TeamServer};
use orchester_core::sharing::MemberRole;

use super::{State, work};

/// How long the tasks show in `team-outage-later` before the server stops answering, and a module's page in
/// `team-module-outage`.
const SHOWN_FOR: Duration = Duration::from_secs(3);

/// Serves Kontur to the person as `state` says, the tasks this computer has by their number keeping their ids `here`;
/// the role the person's link records, as the server gives it.
pub fn serve_as(server: &TeamServer, state: State, here: &HashMap<u32, String>) -> Option<MemberRole> {
  let standing = match state {
    State::Admin | State::AdminWork => Standing::Admin,
    State::ServerAdmin => Standing::ServerAdmin,
    _ => Standing::Member,
  };
  let ids = if state.runs_here() { here } else { &HashMap::new() };
  serve(
    server,
    (state, standing),
    matches!(state, State::Outage | State::OutageRunning),
    ids,
  );
  match state {
    State::OutageLater => server.authority_unavailable_once_read(SHOWN_FOR),
    State::ModuleOutage => server.authority_unavailable_once_module_read(SHOWN_FOR),
    _ => work::plan(state, server),
  }
  match standing {
    Standing::Member => Some(MemberRole::Member),
    Standing::Admin => Some(MemberRole::Owner),
    Standing::ServerAdmin => None,
  }
}

/// Kontur's tasks on the team server: number, title, workflow status, priority and minutes since created. 18 are
/// active, 3 in the backlog and 3 done, as the frame counts them.
const TASKS: [(u32, &str, &str, &str, i64); 24] = [
  (12, "Add retry to webhook sender", "in_progress", "high", 120),
  (13, "Sync contacts with HubSpot", "in_progress", "medium", 1380),
  (10, "Plan the GitHub app permissions", "in_progress", "medium", 1440),
  (9, "Show the signed-in account", "in_review", "low", 1440),
  (11, "Export invoices as PDF", "in_progress", "medium", 180),
  (14, "Cache GitHub API responses", "in_review", "medium", 300),
  (23, "Explain an expired GitHub token", "in_progress", "low", 360),
  (24, "Import contacts from CSV", "in_progress", "low", 1440),
  (16, "Rate-limit messages from GitHub", "in_progress", "medium", 12),
  (21, "Typed errors per module", "in_progress", "low", 4),
  (8, "Localize the invoice emails", "todo", "medium", 1440),
  (17, "Retry failed exports nightly", "todo", "low", 1440),
  (18, "Webhook signing secrets per endpoint", "todo", "medium", 1440),
  (5, "Rotate API keys from settings", "todo", "medium", 2880),
  (19, "Verify webhook signatures on receive", "todo", "medium", 2880),
  (20, "Paginate the audit log", "todo", "low", 2880),
  (22, "Retry webhook delivery on 5xx", "todo", "low", 2880),
  (7, "Close stale HubSpot deals", "in_review", "low", 1440),
  (25, "Dark mode for invoice PDFs", "backlog", "low", 4320),
  (26, "Per-user API rate limits", "backlog", "medium", 5760),
  (27, "Audit log export to CSV", "backlog", "low", 8640),
  (1, "Set up the webhook endpoint", "done", "medium", 5760),
  (2, "Document the API keys page", "done", "low", 7200),
  (15, "Weekly contacts digest", "done", "low", 7200),
];

/// The tasks on the seed's Quick fix pipeline, as the frame draws them; the others are on Feature delivery.
const QUICK_FIXES: [u32; 3] = [9, 17, 23];

/// The task the person claimed and is assigned, and how many minutes ago their run last renewed the claim.
const CLAIMED: (u32, i64) = (12, 2);

/// Serves Kontur's tasks and modules to the person standing as `standing` in `state`, with dana-k as its Admin and olha
/// as the server's admin, the tasks numbered in `ids` with those ids; `unavailable` makes every read of them `503
/// unavailable`.
fn serve(server: &TeamServer, (state, standing): (State, Standing), unavailable: bool, ids: &HashMap<u32, String>) {
  server.stand_as(standing);
  server.members_named(&["dana-k"], &["olha"]);
  let mut modules = modules::modules();
  work::modules(state, &mut modules);
  server.authority_modules(modules);
  server.authority_tasks(
    TASKS
      .iter()
      .map(|&(number, title, workflow, priority, created_minutes_ago)| {
        let mut task = AuthorityTask {
          number,
          title: title.to_owned(),
          workflow,
          priority,
          pipeline: Some(if QUICK_FIXES.contains(&number) {
            "quick-fix"
          } else {
            "feature-delivery"
          }),
          module: modules::of(number),
          created_minutes_ago,
          id: ids.get(&number).cloned(),
          assignee: (number == CLAIMED.0).then_some(Person::You),
          claim: (number == CLAIMED.0).then_some(AuthorityClaim {
            holder: Person::You,
            renewed_minutes_ago: CLAIMED.1,
            stale: false,
            generation: 0,
          }),
          work: (number == CLAIMED.0).then_some("running"),
        };
        work::task(state, &mut task);
        task
      })
      .collect(),
  );
  server.authority_unavailable(unavailable);
  server.require_version((state == State::VersionRefused).then_some(work::REQUIRED_VERSION));
}
