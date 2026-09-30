//! Choosing the records an agent's prompt gets. Only active records of the agent's task and of the
//! project are considered. They come in tiers, in this order: the task's latest handoff, pinned
//! records, the task's decisions and warnings, then the records that match the step's text. Within
//! a tier records are ranked by
//!
//! `score = 0.40·relevance + 0.20·importance + 0.15·recency + 0.15·role + 0.10·confidence`
//!
//! each term from 0 to 1: relevance is the mean of the record's word (BM25) and fragment (trigram
//! BM25) relevance, each divided by the best among the matches; importance and confidence are 0,
//! 0.5 and 1 for low, normal (medium) and high; recency halves every 14 days since the record last
//! changed; role is 1 for a record the same agent profile wrote or one tagged with the agent's
//! role, else 0. When there are more matches than the other tiers leave slots for, a match whose
//! relevance is below 0.2 is not given at all.
//! The ranked records are then taken within the project's limits ([`super::selection`]).

use super::{
  Confidence, Importance, Memory, MemoryAuthor, MemoryKind, MemoryLimits, MemoryMatch, MemoryScope, MemoryStatus,
  selection::{Ranked, Recalled, within_limits},
};
use crate::{
  id::{ResourceId, TaskId},
  time::Timestamp,
};

const RELEVANCE: f64 = 0.40;
const IMPORTANCE: f64 = 0.20;
const RECENCY: f64 = 0.15;
const ROLE: f64 = 0.15;
const CONFIDENCE: f64 = 0.10;
/// A match less relevant than this share of the best match is too loosely related to take a slot
/// another match competes for.
const MIN_RELEVANCE: f64 = 0.2;
const HALF_LIFE_DAYS: f64 = 14.0;
const DAY_MILLIS: f64 = 86_400_000.0;

/// Tokens a text is estimated to take: one per four bytes of UTF-8, rounded up. Orchester counts
/// no tokens itself elsewhere (budgets use what the CLIs report); bytes rather than characters so
/// that Cyrillic and other non-Latin text, which models split finer, is not underestimated.
pub fn estimated_tokens(text: &str) -> u32 {
  u32::try_from(text.len().div_ceil(4)).unwrap_or(u32::MAX)
}

/// Why a record is in a prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InjectionReason {
  /// The task's latest handoff.
  Handoff,
  Pinned,
  /// An active decision of the task.
  Decision,
  /// An active warning of the task.
  Warning,
  /// It matched the step's text.
  Match,
}

impl InjectionReason {
  /// Stable name, as stored.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Handoff => "handoff",
      Self::Pinned => "pinned",
      Self::Decision => "decision",
      Self::Warning => "warning",
      Self::Match => "match",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::Handoff, Self::Pinned, Self::Decision, Self::Warning, Self::Match]
      .into_iter()
      .find(|reason| reason.name() == name)
  }

  /// The order tiers are filled in: decisions and warnings are one tier.
  const fn tier(self) -> u8 {
    match self {
      Self::Handoff => 0,
      Self::Pinned => 1,
      Self::Decision | Self::Warning => 2,
      Self::Match => 3,
    }
  }
}

/// Whom the records are for.
#[derive(Clone, Copy, Debug)]
pub struct RecallRequest<'a> {
  pub task: TaskId,
  /// The agent profile that runs the step.
  pub profile: ResourceId,
  /// Its role, matched against records' tags in lowercase with dashes for spaces.
  pub role: &'a str,
  pub limits: MemoryLimits,
  pub now: Timestamp,
}

/// The records for `request` from `candidates` (the task's active records and the project's pinned
/// ones) and the search's `matches`, as the module describes.
pub fn recall(request: &RecallRequest<'_>, mut candidates: Vec<Memory>, mut matches: Vec<MemoryMatch>) -> Recalled {
  candidates.retain(|memory| reaches(request, memory));
  matches.retain(|found| reaches(request, &found.memory));
  let best_words = matches.iter().map(|found| found.words).fold(0.0, f64::max);
  let best_fragments = matches.iter().map(|found| found.fragments).fold(0.0, f64::max);
  let mut ranked: Vec<(InjectionReason, f64, Memory)> = Vec::new();
  let latest_handoff = candidates
    .iter()
    .filter(|memory| {
      memory.content().kind() == MemoryKind::Handoff && memory.scope() == MemoryScope::Task(request.task)
    })
    .max_by_key(|memory| memory.created_at())
    .map(Memory::id);
  for memory in candidates {
    let reason = if Some(memory.id()) == latest_handoff {
      InjectionReason::Handoff
    } else if memory.is_pinned() {
      InjectionReason::Pinned
    } else if memory.scope() != MemoryScope::Task(request.task) {
      continue;
    } else {
      match memory.content().kind() {
        MemoryKind::Decision => InjectionReason::Decision,
        MemoryKind::Warning => InjectionReason::Warning,
        _ => continue,
      }
    };
    ranked.push((reason, score(request, &memory, 0.0), memory));
  }
  // Matches compete for the slots the other tiers leave; only then is a loose one left out.
  let free = usize::from(request.limits.records()).saturating_sub(ranked.len());
  let compete = matches.len() > free;
  for found in matches {
    let relevance = (ratio(found.words, best_words) + ratio(found.fragments, best_fragments)) / 2.0;
    if compete && relevance < MIN_RELEVANCE {
      continue;
    }
    ranked.push((
      InjectionReason::Match,
      score(request, &found.memory, relevance),
      found.memory,
    ));
  }
  ranked.sort_by(|(left_reason, left_score, left), (right_reason, right_score, right)| {
    left_reason
      .tier()
      .cmp(&right_reason.tier())
      .then(right_score.total_cmp(left_score))
      .then(right.updated_at().cmp(&left.updated_at()))
  });
  let ranked = ranked
    .into_iter()
    .map(|(reason, _, memory)| Ranked {
      reason,
      body: memory.content().text().to_owned(),
      memory,
    })
    .collect();
  let mut recalled = within_limits(request.limits, ranked);
  recalled.drop_repeated_decisions();
  recalled
}

/// Whether `memory` may reach an agent of the request's task at all.
fn reaches(request: &RecallRequest<'_>, memory: &Memory) -> bool {
  memory.status() == MemoryStatus::Active
    && (memory.scope() == MemoryScope::Project || memory.scope() == MemoryScope::Task(request.task))
}

fn ratio(value: f64, best: f64) -> f64 {
  if best > 0.0 {
    (value / best).clamp(0.0, 1.0)
  } else {
    0.0
  }
}

fn score(request: &RecallRequest<'_>, memory: &Memory, relevance: f64) -> f64 {
  let content = memory.content();
  let importance = match content.importance() {
    Importance::Low => 0.0,
    Importance::Normal => 0.5,
    Importance::High => 1.0,
  };
  let age_days = (request.now.unix_millis() - memory.updated_at().unix_millis()).max(0) as f64 / DAY_MILLIS;
  let recency = 0.5_f64.powf(age_days / HALF_LIFE_DAYS);
  let role_tag = request.role.trim().to_lowercase().replace(char::is_whitespace, "-");
  let same_profile = matches!(memory.author(), MemoryAuthor::Agent(agent) if agent.profile == request.profile);
  let role = if same_profile || content.tags().contains(&role_tag) {
    1.0
  } else {
    0.0
  };
  let confidence = match memory.confidence() {
    Confidence::Low => 0.0,
    Confidence::Medium => 0.5,
    Confidence::High => 1.0,
  };
  RELEVANCE * relevance + IMPORTANCE * importance + RECENCY * recency + ROLE * role + CONFIDENCE * confidence
}
