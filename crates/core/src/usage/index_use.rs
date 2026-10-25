//! *Code index use* (ORC-90): every agent invocation that started in the range and ended, by what it did with the code
//! index, those that changed no file too: searched the index it got, got it and never searched it, or worked without
//! it. *Tokens before first edit* counts only invocations that changed a file, so a step that only reads (research,
//! review) shows here alone. An invocation recorded before what it got of the index, or its searches, were kept is left
//! out.

use std::collections::HashMap;

use super::{
  IndexGiven,
  efficiency::median,
  report::{Spender, Within},
};
use crate::id::InvocationId;

/// One group of *Code index use*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexGroup {
  pub invocations: usize,
  /// The median an invocation of the group spent in the range.
  pub median: Option<u64>,
  /// The searches of the index its invocations made.
  pub searches: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexUse {
  /// Got the code index and searched it at least once.
  pub searched: IndexGroup,
  /// Got the code index and never searched it.
  pub not_searched: IndexGroup,
  /// Worked without the code index.
  pub without_index: IndexGroup,
}

impl IndexUse {
  pub(super) fn of(within: &Within<'_>) -> Self {
    let mut spent: HashMap<InvocationId, u64> = HashMap::new();
    for turn in &within.turns {
      if matches!(turn.by, Spender::Agent) {
        *spent.entry(turn.launch.invocation.id).or_default() += turn.usage.counted();
      }
    }
    let mut groups: [(IndexGroup, Vec<u64>); 3] = Default::default();
    for (launch, _) in &within.launches {
      let invocation = launch.invocation;
      let started = within.start <= invocation.started_at && invocation.started_at < within.end;
      let Some(searches) = invocation.edit.as_ref().and_then(|edit| edit.index_searches) else {
        continue;
      };
      if !started || invocation.ended_at.is_none() {
        continue;
      }
      let group = match (invocation.index, searches) {
        (IndexGiven::Yes, 0) => 1,
        (IndexGiven::Yes, _) => 0,
        (IndexGiven::No, _) => 2,
        (IndexGiven::Unknown, _) => continue,
      };
      let (counts, tokens) = &mut groups[group];
      counts.invocations += 1;
      counts.searches += searches;
      tokens.push(spent.get(&invocation.id).copied().unwrap_or(0));
    }
    let [searched, not_searched, without_index] = groups.map(|(mut group, mut tokens)| {
      group.median = median(&mut tokens);
      group
    });
    Self {
      searched,
      not_searched,
      without_index,
    }
  }
}
