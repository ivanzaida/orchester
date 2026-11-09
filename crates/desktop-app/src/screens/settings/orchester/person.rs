//! What only a person does in Settings › Orchester and Project settings › Sharing (N1 security review 1, B2 and M1):
//! what changes who this computer trusts or what it sends ([`Guarded::Sharing`]), and *It matches* under a check code
//! ([`Guarded::CheckCode`]). The button's handler refuses lurq's
//! synthetic input (an MCP client's) and the panel says why under the button with the hand in `warning`, as the
//! Orchestrator's person-only proposals do; reading the window and moving around it stay open to clients.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
  node::Element,
};

use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  ui::tinted_note::person_only,
};

/// The person-only action that synthetic input pressed last in a panel, by its button's id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused(Option<String>);

impl DevtoolsInspectable for Refused {}

/// The person-only actions of one panel. Clones share which one was refused.
#[derive(Clone)]
pub struct PersonOnly {
  gate: Option<InputGate>,
  refused: Signal<Refused>,
}

impl PersonOnly {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      gate: ctx.use_context::<InputGate>(),
      refused: ctx.signal(Refused(None)),
    }
  }

  /// Whether the input being delivered may take the action whose button is `id`: a person's always, synthetic input
  /// never, and then the panel says so under that button. Read in the handler, while lurq delivers the input.
  pub fn allows(&self, id: &str) -> bool {
    self.allows_as(id, Guarded::Sharing)
  }

  /// [`Self::allows`] for *It matches* under a check code, which only a person compares with the server's console.
  pub fn confirms_code(&self, id: &str) -> bool {
    self.allows_as(id, Guarded::CheckCode)
  }

  fn allows_as(&self, id: &str, guarded: Guarded) -> bool {
    let allowed = input_may_decide(self.gate.as_ref(), guarded);
    self.refused.set(Refused((!allowed).then(|| id.to_owned())));
    allowed
  }

  /// Why the action whose button is `id` did nothing, once synthetic input pressed it.
  pub fn reason(&self, ctx: &Ctx, id: &str) -> Option<Element> {
    (self.refused.get().0.as_deref() == Some(id))
      .then(|| person_only(format!("{id}-person-only"), &ctx.t("cloud.personOnly")).into())
  }
}
