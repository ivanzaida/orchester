//! Taking ranked records into a prompt within the project's limits. Records come in rank order; a
//! record already taken, one with the key of a taken one and one saying the same as a taken one are
//! skipped. The task's latest handoff is always taken: when it alone exceeds the token limit its
//! body is cut to fit, with a marker saying so. Every other record is taken while the record limit
//! allows and its tokens fit; what is left out, and why, is kept with what was given, so an
//! invocation's record says what its prompt did not get.

use std::collections::BTreeSet;

use super::{InjectedMemory, InjectionReason, Memory, MemoryLimits, estimated_tokens, render::entry_text};
use crate::id::MemoryId;

/// What became of a record chosen for a prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Delivery {
  Given,
  /// Given cut to fit the token limit.
  Truncated,
  /// Left out: the prompt had all the records the limit allows.
  OverRecordLimit,
  /// Left out: its tokens did not fit what the limit left.
  OverTokenLimit,
}

impl Delivery {
  /// Stable name, as stored.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Given => "given",
      Self::Truncated => "truncated",
      Self::OverRecordLimit => "over_record_limit",
      Self::OverTokenLimit => "over_token_limit",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [
      Self::Given,
      Self::Truncated,
      Self::OverRecordLimit,
      Self::OverTokenLimit,
    ]
    .into_iter()
    .find(|delivery| delivery.name() == name)
  }

  pub const fn is_given(self) -> bool {
    matches!(self, Self::Given | Self::Truncated)
  }
}

/// A record given to a prompt, why, the body the prompt shows (cut, when it was truncated), and the
/// text the prompt shows for it ([`super::render`]).
#[derive(Clone, Debug, PartialEq)]
pub struct RecalledMemory {
  pub memory: Memory,
  pub reason: InjectionReason,
  pub body: String,
  pub text: String,
  pub tokens: u32,
  pub truncated: bool,
}

/// A record chosen for a prompt that did not fit it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeftOut {
  pub memory: MemoryId,
  pub reason: InjectionReason,
  pub tokens: u32,
  pub delivery: Delivery,
}

/// The records a prompt gets, in prompt order, and those left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recalled {
  entries: Vec<RecalledMemory>,
  left_out: Vec<LeftOut>,
}

impl Recalled {
  pub fn entries(&self) -> &[RecalledMemory] {
    &self.entries
  }

  /// Ranked records the limits left out, in rank order.
  pub fn left_out(&self) -> &[LeftOut] {
    &self.left_out
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// The estimated tokens of the records given.
  pub fn tokens(&self) -> u32 {
    self.entries.iter().map(|entry| entry.tokens).sum()
  }

  /// What to record with the invocation: the records given, in prompt order, then those left out.
  pub fn injected(&self) -> Vec<InjectedMemory> {
    let given = self.entries.iter().map(|entry| InjectedMemory {
      memory: entry.memory.id(),
      reason: entry.reason,
      tokens: entry.tokens,
      delivery: if entry.truncated {
        Delivery::Truncated
      } else {
        Delivery::Given
      },
    });
    let left_out = self.left_out.iter().map(|left| InjectedMemory {
      memory: left.memory,
      reason: left.reason,
      tokens: left.tokens,
      delivery: left.delivery,
    });
    given.chain(left_out).collect()
  }
}

/// A record as ranked: why it was chosen, and the body the prompt would show.
pub(super) struct Ranked {
  pub(super) reason: InjectionReason,
  pub(super) memory: Memory,
  pub(super) body: String,
}

/// The ranked records within `limits`, as the module describes.
pub(super) fn within_limits(limits: MemoryLimits, ranked: Vec<Ranked>) -> Recalled {
  let mut seen = Seen::default();
  let mut recalled = Recalled::default();
  let mut tokens = 0_u32;
  for Ranked { reason, memory, body } in ranked {
    if !seen.first(&memory) {
      continue;
    }
    let text = entry_text(&memory, reason, &body);
    let cost = estimated_tokens(&text);
    let delivery = if recalled.entries.len() >= usize::from(limits.records()) {
      Delivery::OverRecordLimit
    } else if tokens.saturating_add(cost) <= limits.tokens() {
      Delivery::Given
    } else if reason == InjectionReason::Handoff {
      Delivery::Truncated
    } else {
      Delivery::OverTokenLimit
    };
    let entry = match delivery {
      Delivery::Given => RecalledMemory {
        memory,
        reason,
        body,
        text,
        tokens: cost,
        truncated: false,
      },
      Delivery::Truncated => truncated(memory, &body, limits.tokens() - tokens),
      Delivery::OverRecordLimit | Delivery::OverTokenLimit => {
        recalled.left_out.push(LeftOut {
          memory: memory.id(),
          reason,
          tokens: cost,
          delivery,
        });
        continue;
      }
    };
    tokens += entry.tokens;
    recalled.entries.push(entry);
  }
  recalled
}

/// The ids, keys and texts of the records taken so far.
#[derive(Default)]
struct Seen {
  ids: BTreeSet<MemoryId>,
  keys: BTreeSet<String>,
  texts: BTreeSet<String>,
}

impl Seen {
  /// Whether `memory` is the first of its id, key and text; it counts as seen from now on.
  fn first(&mut self, memory: &Memory) -> bool {
    let content = memory.content();
    let text = content
      .text()
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" ")
      .to_lowercase();
    let key = content.key().map(|key| key.as_str().to_owned());
    if self.ids.contains(&memory.id())
      || key.as_ref().is_some_and(|key| self.keys.contains(key))
      || self.texts.contains(&text)
    {
      return false;
    }
    self.ids.insert(memory.id());
    self.keys.extend(key);
    self.texts.insert(text);
    true
  }
}

/// The latest handoff with its body cut, at a line break where one is near, so that it fits
/// `budget` tokens with a marker saying how much is shown.
fn truncated(memory: Memory, body: &str, budget: u32) -> RecalledMemory {
  let total = body.chars().count();
  let mut shown = usize::try_from(budget)
    .map_or(total, |budget| budget.saturating_mul(4))
    .min(total);
  loop {
    let cut: String = body.chars().take(shown).collect();
    let cut = match cut.rfind('\n') {
      Some(line) if line * 5 > cut.len() * 4 => cut[..line].to_owned(),
      _ => cut,
    };
    let cut_body = format!(
      "{}\n[Cut to fit the memory limit: {} of {total} characters shown.]",
      cut.trim_end(),
      cut.chars().count()
    );
    let text = entry_text(&memory, InjectionReason::Handoff, &cut_body);
    let cost = estimated_tokens(&text);
    if cost <= budget || shown == 0 {
      return RecalledMemory {
        memory,
        reason: InjectionReason::Handoff,
        body: cut_body,
        text,
        tokens: cost,
        truncated: true,
      };
    }
    shown = shown.saturating_sub((shown / 10).max(1));
  }
}
