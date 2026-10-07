//! A native agent's tool calls in the transcript (04.H10): what a call did ([`NativeCall`]) marks the
//! entry of its command, which then reads as a file tool's row; a file change that a native edit's own row
//! shows is not shown again; and a command a person or a rule approved gets an entry of its own for the
//! approval, before the command's.

use std::sync::Arc;

use orchester_core::{
  connection::stream::{CallDetail, NativeCall},
  id::InvocationId,
  run::ActivityEntry,
};

use super::{CommandMark, DecidedMark, Entry, EntryKind};

/// Native calls told before the entry that ends them exists: a command that did not run, a call whose
/// input could not be read.
#[derive(Default)]
pub(super) struct Told(Vec<(InvocationId, Arc<str>, NativeCall)>);

impl Told {
  /// Marks the running entry of call `item` with what it did, or keeps that for the entry its end makes.
  pub(super) fn note(&mut self, entries: &mut [Entry], stored: &ActivityEntry, item: &str, call: &NativeCall) {
    let running = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
      EntryKind::Command(command)
        if entry.invocation == stored.invocation && *command.item == *item && command.status.is_none() =>
      {
        Some(command)
      }
      _ => None,
    });
    match running {
      Some(command) => command.native = Some(call.clone()),
      None => self.0.push((stored.invocation, item.into(), call.clone())),
    }
  }

  /// What call `item` of `invocation` did, told before its entry existed.
  pub(super) fn take(&mut self, invocation: InvocationId, item: &str) -> Option<NativeCall> {
    let at = self
      .0
      .iter()
      .position(|(told, told_item, _)| *told == invocation && **told_item == *item)?;
    Some(self.0.remove(at).2)
  }
}

/// Whether the file change of call `item` is shown by the row of the native edit that made it.
pub(super) fn shown_by_edit(entries: &[Entry], stored: &ActivityEntry, item: &str) -> bool {
  entries.iter().rev().any(|entry| match &entry.kind {
    EntryKind::Command(command) => {
      entry.invocation == stored.invocation
        && *command.item == *item
        && matches!(
          command.native,
          Some(NativeCall {
            detail: Some(CallDetail::Changed { .. }),
            ..
          })
        )
    }
    _ => false,
  })
}

/// A native command's approval, as an entry of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalEntry {
  pub mark: DecidedMark,
  /// The command it approved.
  pub command: Arc<str>,
}

/// `entries` with each approved native command's approval taken off its row into an entry of its own,
/// right before the command's.
pub(super) fn approvals_apart(entries: Vec<Entry>) -> Vec<Entry> {
  let mut apart = Vec::with_capacity(entries.len());
  for mut entry in entries {
    let approval = match &mut entry.kind {
      EntryKind::Command(command) if command.native.is_some() => match command.mark.take() {
        Some(CommandMark::Decided(mark)) if mark.decision.decided_by.approves() => Some(ApprovalEntry {
          mark,
          command: command.command.clone(),
        }),
        other => {
          command.mark = other;
          None
        }
      },
      _ => None,
    };
    if let Some(approval) = approval {
      apart.push(Entry {
        sequence: approval.mark.sequence,
        part: 0,
        execution: entry.execution,
        invocation: entry.invocation,
        at: approval.mark.at,
        kind: EntryKind::Approval(approval),
      });
    }
    apart.push(entry);
  }
  apart
}
