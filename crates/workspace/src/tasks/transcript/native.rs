//! A native agent's tool calls in the transcript (04.H10): what a call did ([`NativeCall`]) marks the
//! entry of its command, which then reads as a file tool's row; a CLI's call of Orchester's same file tool
//! (Claude Code's command, Codex's MCP call) is marked with what its input asked, and reads the same way; a file
//! change that the row of the call that made it shows (a native edit's, or one of Orchester's edit tools a CLI
//! called, with its diff) is not shown again; and a native command a person or a rule approved gets an entry of its
//! own for the approval, before the command's.

use std::sync::Arc;

use orchester_core::{connection::stream::NativeCall, id::InvocationId, run::ActivityEntry};

use super::{CommandMark, DecidedMark, Entry, EntryKind};

/// Native calls told before the entry that ends them exists: a command that did not run, a call whose
/// input could not be read.
#[derive(Default)]
pub(super) struct Told(Vec<(InvocationId, Arc<str>, NativeCall)>);

impl Told {
  /// Marks the running entry of call `item` (a command, or a Codex MCP call) with what it did, or keeps that for
  /// the entry its end makes.
  pub(super) fn note(&mut self, entries: &mut [Entry], stored: &ActivityEntry, item: &str, call: &NativeCall) {
    let running = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
      _ if entry.invocation != stored.invocation => None,
      EntryKind::Command(command) if *command.item == *item && command.status.is_none() => Some(&mut command.native),
      EntryKind::McpTool(mcp) if *mcp.item == *item && mcp.status.is_none() => Some(&mut mcp.native),
      _ => None,
    });
    match running {
      Some(native) => *native = Some(call.clone()),
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

/// Whether the file change of call `item` is shown by the row of the call that made it: a native edit's,
/// or that of Orchester's `edit_file` or `write_file`, a command for Claude Code and an MCP call for Codex.
/// Only those calls report a change after a row of their own.
pub(super) fn shown_by_call(entries: &[Entry], stored: &ActivityEntry, item: &str) -> bool {
  entries.iter().rev().any(|entry| {
    entry.invocation == stored.invocation
      && match &entry.kind {
        EntryKind::Command(command) => *command.item == *item,
        EntryKind::McpTool(call) => *call.item == *item,
        _ => false,
      }
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
      // A CLI's call keeps its mark on its row, as every call of an MCP tool does.
      EntryKind::Command(command) if command.native.as_ref().is_some_and(|call| call.input.is_none()) => {
        match command.mark.take() {
          Some(CommandMark::Decided(mark)) if mark.decision.decided_by.approves() => Some(ApprovalEntry {
            mark,
            command: command.command.clone(),
          }),
          other => {
            command.mark = other;
            None
          }
        }
      }
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
