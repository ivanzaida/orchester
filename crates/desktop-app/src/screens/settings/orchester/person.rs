//! What only a person does in Settings › Orchester and Project settings › Sharing (N1 security review 1, B2 and M1):
//! what changes who this computer trusts or what it sends ([`Guarded::Sharing`]), and *It matches* under a check code
//! ([`Guarded::CheckCode`]). The button's handler refuses lurq's synthetic input (an MCP client's) to the client that
//! sent it, as its tool call's error, and leaves the window as it was ([`may_press`]); reading the window and moving
//! around it stay open to clients.

use lurq::app::ctx::Ctx;

use crate::mcp::input_gate::{Guarded, InputGate, may_press};

/// The person-only actions of one panel.
#[derive(Clone)]
pub struct PersonOnly {
  gate: Option<InputGate>,
}

impl PersonOnly {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      gate: ctx.use_context::<InputGate>(),
    }
  }

  /// Whether the input being delivered may take `action`: a person's always; synthetic input never, refused to the
  /// client that sent it. `action` is worded after "Only a person can". Read in the handler, while lurq delivers the
  /// input.
  pub fn allows(&self, action: &'static str) -> bool {
    may_press(self.gate.as_ref(), Guarded::Sharing, action)
  }

  /// [`Self::allows`] for *It matches* under a check code, which only a person compares with the server's console.
  pub fn confirms_code(&self) -> bool {
    may_press(
      self.gate.as_ref(),
      Guarded::CheckCode,
      "confirm that a check code matches the one on the server's console",
    )
  }
}
