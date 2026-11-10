//! A press only a person makes is refused to the automation that made it, and the window does not change (the
//! person, 2026-11-10): no banner, no note, no field, nothing a person must clear. The handler that finds lurq's
//! synthetic input on such a press does nothing but [`refuse_press`]; the gate keeps the refusal until the MCP tool
//! call that delivered the input answers, and that call answers with an error naming the action
//! ([`GuardedTools`](super::GuardedTools)). Synthetic input that no MCP call delivered (a test's, through
//! `with_synthetic_input`) has nobody to answer: its refusal is logged and dropped with the next call.

use orchester_core::run::Answerer;

use super::{Guarded, InputGate, input_may_decide};

/// A person-only press synthetic input made, kept until the tool call that delivered it answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefusedPress {
  action: &'static str,
  policy: Guarded,
}

impl RefusedPress {
  /// What the refused client reads: the action, that a person makes it, and how a person can let automation make it
  /// where a setting does.
  pub fn words(self) -> String {
    let allow = match self.policy {
      Guarded::PermissionWidening => {
        "A person can let automation do it: Settings › Automation › Let automation change agent permissions."
      }
      Guarded::ApprovalWidening => {
        "A person can let automation do it: Settings › Automation › Let automation change agent permissions, with Let \
         automation answer approvals."
      }
      Guarded::StepAccept | Guarded::Decision(_) => {
        "A person can let automation do it: Settings › Automation › Let automation answer approvals."
      }
      Guarded::PermissionsSetting
      | Guarded::WideningQuestion
      | Guarded::LocalIntegrationQuestion
      | Guarded::McpServer
      | Guarded::Accounts
      | Guarded::CheckCode
      | Guarded::Sharing
      | Guarded::Person => "No setting lets automation do it.",
    };
    format!(
      "Only a person can {} in the window: the press was refused and the window was left as it was. {allow}",
      self.action
    )
  }

  /// The action refused, as the client's error names it.
  pub const fn action(self) -> &'static str {
    self.action
  }
}

impl InputGate {
  /// Refuses the press being delivered to the MCP client whose tool call delivers it. Called in the handler, while
  /// lurq delivers the input; the handler then leaves the window as it was.
  pub fn refuse_press(&self, action: &'static str, policy: Guarded) {
    *self.0.refused_press.lock() = Some(RefusedPress { action, policy });
  }

  /// The press refused since it was last taken, which the tool call that delivered the input answers with; a test
  /// that delivers synthetic input itself reads it here.
  pub fn take_refused_press(&self) -> Option<RefusedPress> {
    self.0.refused_press.lock().take()
  }
}

/// Refuses the press being delivered as `action` to the automation that made it ([`InputGate::refuse_press`]), and
/// logs it. `action` is what a person does, worded after "Only a person can", e.g. "save a team instruction".
pub fn refuse_press(gate: Option<&InputGate>, policy: Guarded, action: &'static str) {
  tracing::info!(action, ?policy, "a person-only press refused to automation");
  if let Some(gate) = gate {
    gate.refuse_press(action, policy);
  }
}

/// Whether the press being delivered may take `action`, classified as `policy` ([`input_may_decide`]); when it may
/// not, it is refused to the automation that made it ([`refuse_press`]) and the handler leaves the window as it was.
pub fn may_press(gate: Option<&InputGate>, policy: Guarded, action: &'static str) -> bool {
  let may = input_may_decide(gate, policy);
  if !may {
    refuse_press(gate, policy, action);
  }
  may
}

/// Whether the press being delivered may take `action`, which only a person takes and no setting opens to automation:
/// a person's input always, lurq's synthetic input never ([`refuse_press`]).
pub fn persons_press(gate: Option<&InputGate>, action: &'static str) -> bool {
  may_press(gate, Guarded::Person, action)
}

/// Who gives an answer classified as `policy` with the press being delivered ([`super::answerer`]); `None`, with the
/// press refused to the automation that made it, when only a person gives it.
pub fn press_answerer(gate: Option<&InputGate>, policy: Guarded, action: &'static str) -> Option<Answerer> {
  let by = super::answerer(gate, policy);
  if by.is_none() {
    refuse_press(gate, policy, action);
  }
  by
}
