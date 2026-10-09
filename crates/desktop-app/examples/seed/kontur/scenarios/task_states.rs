//! `task-states`: one task in each state the task page's Summary shows (`Task page` `iwrwB`), in key order
//! as listed below (KON-1 is a draft ready to queue, KON-18 a draft that misses details); runs use Feature
//! delivery, so their steps are those of the design's KON-12. The seed prints which key is which state.

use orchester_core::run::RunLimit;

use crate::kontur::{
  Kontur, SeedError,
  shapes::{self, Shape},
};

const STATES: [(&str, &str, Shape); 18] = [
  (
    "Rotate API keys from settings",
    "Let an admin rotate the API key without downtime.",
    Shape::ReadyDraft,
  ),
  (
    "Localize the invoice emails",
    "Send invoice emails in the customer's language.",
    Shape::Queued,
  ),
  (
    "Add retry to webhook sender",
    "Retry failed webhook deliveries with backoff and jitter.",
    Shape::Running("implement"),
  ),
  (
    "Paginate the events API",
    "Page the events endpoint with a cursor.",
    Shape::Paused("implement"),
  ),
  (
    "Explain an expired GitHub token in the status bar",
    "Say in the status bar when the GitHub token expired.",
    Shape::AtLimit("implement", RunLimit::Tokens),
  ),
  (
    "Retry failed exports nightly",
    "Retry exports that failed during the day once at night.",
    DAILY_BUDGET,
  ),
  (
    "Import contacts from CSV",
    "Import contacts from a CSV file with a preview.",
    Shape::Interrupted("implement"),
  ),
  (
    "Export invoices as PDF",
    "Export a month of invoices as one PDF per customer.",
    Shape::Failed("run-checks"),
  ),
  (
    "Retry Slack notifications",
    "Retry Slack notifications the way webhooks are retried.",
    Shape::Cancelled("implement"),
  ),
  (
    "Sync contacts with HubSpot",
    "Sync contacts both ways with HubSpot.",
    Shape::Question("implement"),
  ),
  (
    "Add serde to the webhook payloads",
    "Serialize webhook payloads with serde.",
    Shape::Approval("implement"),
  ),
  (
    "Rate-limit messages from GitHub",
    "Respect GitHub's rate limits when the sync sends messages.",
    Shape::PlanReview,
  ),
  (
    "Back off when GitHub rate-limits the sync",
    "Queue sync messages while GitHub's limit is spent.",
    Shape::PlanRevised,
  ),
  (
    "Cache GitHub API responses",
    "Cache GitHub API responses with their ETags.",
    Shape::Stuck("run-checks"),
  ),
  (
    "Show the signed-in account",
    "Show who is signed in at the top of the sidebar.",
    Shape::Succeeded,
  ),
  (
    "Set up CI caching",
    "Cache cargo's registry and target folder between CI runs.",
    Shape::Merged,
  ),
  (
    "Typed errors per module",
    "Give every module its own error enum.",
    Shape::Unreadable("implement"),
  ),
  ("Docs: connecting GitHub", "", Shape::Draft),
];

const DAILY_BUDGET: Shape = Shape::AtLimit("implement", RunLimit::DailyBudget);
const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  for (index, (title, brief, shape)) in STATES.into_iter().enumerate() {
    let hours = i64::try_from(STATES.len() - index).expect("a short list") * 60;
    // The daily budget's stop is today's, so it is today's budget that is used up.
    let minutes = if shape == DAILY_BUDGET { 40 } else { hours };
    let (created, started) = (kontur.ago(minutes + 30), kontur.ago(minutes));
    let (criteria, pipeline): (&[&str], _) = if shape == Shape::Draft {
      (&[], None)
    } else {
      (&CRITERIA, Some(&catalogue.feature_delivery))
    };
    let mut task = kontur.task((title, brief, criteria), pipeline, created)?;
    shapes::apply(kontur, &mut task, pipeline, shape, started)?;
  }
  kontur.spend_daily_budget()
}
