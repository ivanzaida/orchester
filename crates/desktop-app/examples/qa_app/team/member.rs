//! The member, role and outage states (ORC-716): Kontur's tasks as the team server reads them, with the person a member
//! (or an Admin, or the server's admin) and the Admins named as `Row 1 · Member views` (`rD8mt`) names them; in the
//! outage the server's reads are unavailable, from the start or once the tasks were shown. Fixture data for QA, never
//! shipped.

use std::time::Duration;

use orchester_api::test_support::team_server::{AuthorityTask, Standing, TeamServer};
use orchester_core::sharing::MemberRole;

use super::State;

/// How long the tasks show in `team-outage-later` before the server stops answering.
const SHOWN_FOR: Duration = Duration::from_secs(3);

/// Serves Kontur to the person as `state` says; the role the person's link records, as the server gives it.
pub fn serve_as(server: &TeamServer, state: State) -> Option<MemberRole> {
  let standing = match state {
    State::Admin => Standing::Admin,
    State::ServerAdmin => Standing::ServerAdmin,
    _ => Standing::Member,
  };
  serve(server, standing, matches!(state, State::Outage | State::OutageRunning));
  if state == State::OutageLater {
    server.authority_unavailable_once_read(SHOWN_FOR);
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

/// Serves Kontur's tasks to the person standing as `standing`, with dana-k as its Admin and olha as the server's admin;
/// `unavailable` makes every read of them `503 unavailable`.
fn serve(server: &TeamServer, standing: Standing, unavailable: bool) {
  server.stand_as(standing);
  server.members_named(&["dana-k"], &["olha"]);
  server.authority_tasks(
    TASKS
      .iter()
      .map(
        |&(number, title, workflow, priority, created_minutes_ago)| AuthorityTask {
          number,
          title: title.to_owned(),
          workflow,
          priority,
          pipeline: Some(if QUICK_FIXES.contains(&number) {
            "quick-fix"
          } else {
            "feature-delivery"
          }),
          module: number % 2 == 0,
          created_minutes_ago,
        },
      )
      .collect(),
  );
  server.authority_unavailable(unavailable);
}
