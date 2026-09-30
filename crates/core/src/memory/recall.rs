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
//! role, else 0.
//! A record already chosen, one with the key of a chosen one and one saying the same as a chosen
//! one are left out. Records are then taken in order while the project's record limit allows,
//! skipping any that would exceed its token limit.

use std::collections::BTreeSet;

use super::{
  Confidence, Importance, InjectedMemory, Memory, MemoryAuthor, MemoryKind, MemoryLimits, MemoryMatch, MemoryScope,
  MemoryStatus,
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

/// A record chosen for a prompt, why, and the text the prompt shows for it.
#[derive(Clone, Debug, PartialEq)]
pub struct RecalledMemory {
  pub memory: Memory,
  pub reason: InjectionReason,
  pub text: String,
  pub tokens: u32,
}

/// The records a prompt gets, in prompt order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recalled {
  entries: Vec<RecalledMemory>,
}

impl Recalled {
  pub fn entries(&self) -> &[RecalledMemory] {
    &self.entries
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// The estimated tokens of all the records.
  pub fn tokens(&self) -> u32 {
    self.entries.iter().map(|entry| entry.tokens).sum()
  }

  /// What to record with the invocation.
  pub fn injected(&self) -> Vec<InjectedMemory> {
    self
      .entries
      .iter()
      .map(|entry| InjectedMemory {
        memory: entry.memory.id(),
        reason: entry.reason,
        tokens: entry.tokens,
      })
      .collect()
  }
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
  for found in matches {
    let relevance = (ratio(found.words, best_words) + ratio(found.fragments, best_fragments)) / 2.0;
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
  Recalled {
    entries: within_limits(request.limits, ranked),
  }
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

/// The ranked records, each once, within the record and token limits.
fn within_limits(limits: MemoryLimits, ranked: Vec<(InjectionReason, f64, Memory)>) -> Vec<RecalledMemory> {
  let mut ids = BTreeSet::new();
  let mut keys = BTreeSet::new();
  let mut texts = BTreeSet::new();
  let mut tokens = 0_u32;
  let mut chosen = Vec::new();
  for (reason, _, memory) in ranked {
    if chosen.len() >= usize::from(limits.records()) {
      break;
    }
    let same_text = memory
      .content()
      .text()
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" ")
      .to_lowercase();
    let key = memory.content().key().cloned();
    if ids.contains(&memory.id()) || key.as_ref().is_some_and(|key| keys.contains(key)) || texts.contains(&same_text) {
      continue;
    }
    let text = entry_text(&memory, reason);
    let cost = estimated_tokens(&text);
    if tokens.saturating_add(cost) > limits.tokens() {
      continue;
    }
    tokens += cost;
    ids.insert(memory.id());
    keys.extend(key);
    texts.insert(same_text);
    chosen.push(RecalledMemory {
      memory,
      reason,
      text,
      tokens: cost,
    });
  }
  chosen
}

/// How the prompt shows a record: a handoff as its text under its own heading, any other record as
/// a list item with its kind and key.
fn entry_text(memory: &Memory, reason: InjectionReason) -> String {
  let content = memory.content();
  if reason == InjectionReason::Handoff {
    return content.text().to_owned();
  }
  let key = content
    .key()
    .map(|key| format!(" ({})", key.as_str()))
    .unwrap_or_default();
  let text = content.text().lines().collect::<Vec<_>>().join("\n  ");
  format!("- {}{key}: {text}", content.kind().label())
}
