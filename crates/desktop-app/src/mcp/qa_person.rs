//! The QA gate (ORC-899): in a QA copy, and only there, the app's MCP client counts as the person for the
//! Orchestrator's person-only actions, so a verifier reaches what a person's press reaches. A synthetic press (lurq's
//! `lurq_interact`, an MCP client's) of *Stop watching*, *Take it here* or *Leave it there*, *Start* on a proposed run,
//! *Approve and do it*, *Apply* on a proposed change, going back, deleting a conversation, and a command applied from
//! the composer (a `/watch-module`, `/watch-task` or `/unwatch` badge sent with Enter) go through as a person's, and a
//! message it sends is the person's. Outside a QA copy such a press is refused to the client that made it
//! ([`QaPerson::refuses`]), and the window stays as it was.
//!
//! Only `examples/qa_app` (the program `tools/qa-app.ps1` starts) turns it on, once, before its window opens
//! ([`QaPerson::count_mcp_client_as_person`]); the installed app (`src/main.rs`), a normal debug start and the tests
//! that do not ask for it keep refusing synthetic input there. It is no environment variable and nothing stored: a
//! setting another program could put in front of the installed app would be a way around the refusal. It widens nothing
//! else: Settings, permission and approval questions, secrets, sharing and the task pages keep their own refusals
//! ([`super::input_gate`]). `docs/stable-build.md` (QA copies) and `.claude/skills/design-verification/SKILL.md` say
//! what it covers.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx, events::is_synthetic_input};

use crate::{
  mcp::input_gate::{Guarded, InputGate, refuse_press},
  services::Services,
};

/// Whether this session's MCP client counts as the person for the Orchestrator's person-only actions. Clones share it;
/// a new session's is off.
#[derive(Clone, Default)]
pub struct QaPerson {
  counts: Arc<AtomicBool>,
  /// The window's input gate, which refuses a person-only press to the client that made it ([`Self::refuses`]); `None`
  /// in the session's services, which [`Self::of`] fills.
  gate: Option<InputGate>,
}

impl PartialEq for QaPerson {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.counts, &other.counts) && self.gate == other.gate
  }
}

impl std::fmt::Debug for QaPerson {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("QaPerson")
      .field("counts", &self.counts())
      .finish_non_exhaustive()
  }
}

impl DevtoolsInspectable for QaPerson {}

impl QaPerson {
  /// The session's gate, with the window's input gate; off where no session's services are provided.
  pub fn of(ctx: &mut Ctx) -> Self {
    let qa_person = ctx
      .use_context::<Services>()
      .map(|services| services.qa_person)
      .unwrap_or_default();
    Self {
      gate: ctx.use_context::<InputGate>(),
      ..qa_person
    }
  }

  /// Makes the MCP client count as the person, for the rest of the session. A QA copy's alone: only
  /// `examples/qa_app` calls it.
  pub fn count_mcp_client_as_person(&self) {
    self.counts.store(true, Ordering::SeqCst);
  }

  /// Whether the MCP client counts as the person.
  pub fn counts(&self) -> bool {
    self.counts.load(Ordering::SeqCst)
  }

  /// Whether the input being delivered is refused by a person-only action: lurq's synthetic input, outside a QA copy.
  /// Read in the handler, while lurq delivers the input.
  pub fn refuses_input(&self) -> bool {
    is_synthetic_input() && !self.counts()
  }

  /// [`Self::refuses_input`] for the person-only `action`, refused to the client that sent the input when it is; the
  /// handler then leaves the window as it was. `action` is worded after "Only a person can".
  pub fn refuses(&self, action: &'static str) -> bool {
    let refused = self.refuses_input();
    if refused {
      refuse_press(self.gate.as_ref(), Guarded::Person, action);
    }
    refused
  }
}
