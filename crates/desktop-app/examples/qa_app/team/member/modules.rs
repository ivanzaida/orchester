//! Kontur's modules on the team server, as `01d · Modules · member` (`rvXxg`) and `01e · Module · member` (`FuHYN`)
//! draw them: Webhooks v2 held by olha, Billing exports held by the person, and API rate limits held by nobody, each
//! with its tasks. Fixture data for QA, never shipped.

use orchester_api::test_support::team_server::{AuthorityClaim, AuthorityModule, Person};

/// The tasks in each module, by number, in the order of [`modules`].
const TASKS: [&[u32]; 3] = [&[1, 12, 18, 19, 22], &[8, 11, 17, 25, 27], &[2, 5, 26]];

/// The modules, oldest first.
pub fn modules() -> Vec<AuthorityModule> {
  vec![
    AuthorityModule {
      name: "Webhooks v2",
      description: "Retries, signing and the delivery log. Failed webhook deliveries retry with backoff, every \
                    endpoint signs its payloads with its own secret, and the delivery log keeps each attempt so a \
                    failed delivery can be replayed.",
      start: Some("2026-10-14"),
      target: Some("2026-11-01"),
      branch: "module/webhooks-v2",
      assignee: Some(Person::Named("olha")),
      claim: Some(AuthorityClaim {
        holder: Person::Named("olha"),
        renewed_minutes_ago: 4,
        stale: false,
      }),
      likely: Some(("2026-10-30", "2026-11-02")),
    },
    AuthorityModule {
      name: "Billing exports",
      description: "CSV and PDF exports for invoices",
      start: Some("2026-10-20"),
      target: Some("2026-11-08"),
      branch: "module/billing-exports",
      assignee: Some(Person::You),
      claim: Some(AuthorityClaim {
        holder: Person::You,
        renewed_minutes_ago: 6,
        stale: false,
      }),
      likely: Some(("2026-11-12", "2026-11-16")),
    },
    AuthorityModule {
      name: "API rate limits",
      description: "Per-user limits and headers",
      start: None,
      target: None,
      branch: "module/api-rate-limits",
      assignee: None,
      claim: None,
      likely: None,
    },
  ]
}

/// The module the task numbered `number` is in.
pub fn of(number: u32) -> Option<usize> {
  TASKS.iter().position(|tasks| tasks.contains(&number))
}
