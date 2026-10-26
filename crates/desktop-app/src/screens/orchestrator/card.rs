//! What the Orchestrator's decision cards share: the note that the person approved a proposal in the chat, and the
//! person's decision on a proposal, taken off the event loop.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    i18n::I18n,
    theme::PaletteColor,
  },
  components::Row,
};
use orchester_core::orchestrator::ChatAction;
use orchester_workspace::{
  messages::{self, Message},
  orchestrator::{
    proposal::{self, Decision},
    tools::ChatScope,
  },
};

use crate::{
  mcp::domain,
  services::Services,
  ui::{icon::Icon, tinted_note::tinted_note},
};

/// The note of card `id` that the person approved its proposal in the chat, quoting the message that did
/// (`{id}-in-chat`, ORC-96); `None` for one decided otherwise.
pub(super) fn approved_in_chat(ctx: &Ctx, id: &str, action: &ChatAction) -> Option<Row> {
  let approval = action.approved_in.as_ref()?;
  Some(tinted_note(
    format!("{id}-in-chat"),
    (Icon::MessageSquare, PaletteColor::TextMuted),
    &ctx.t_args(
      "orchestrator.change.approvedInChat",
      [("words", approval.words.as_str())],
    ),
  ))
}

/// A decision to take on a proposal.
#[derive(Clone)]
pub(in crate::screens) struct Deciding {
  pub(in crate::screens) scope: ChatScope,
  pub(in crate::screens) seq: i64,
  pub(in crate::screens) decision: Decision,
}

impl DevtoolsInspectable for Deciding {}

pub(in crate::screens) type DecideAction = FutureAction<Deciding, (), Message>;

/// The person's decisions on a card's proposal, carried out through the domain tools of the session. Each is spawned
/// on the session's runtime when it is taken, so it is carried out and its outcome kept even when the card goes away
/// (another tab, another page) before it ends; the card only waits for it. A task's conversation takes its decisions
/// on the same proposals the same way (ORC-45).
pub(in crate::screens) fn decide_action(ctx: &mut Ctx) -> DecideAction {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  ctx.future_action(move |deciding: Deciding| {
    let decided = services.runtime.spawn(decide(services.clone(), i18n.clone(), deciding));
    async move { decided.await.map_err(|_| messages::interrupted())? }
  })
}

/// Takes `deciding` through the domain tools of `services`, explaining a refusal in the words of `i18n`.
async fn decide(services: Services, i18n: I18n, deciding: Deciding) -> Result<(), Message> {
  let tools = domain::on(&services);
  let translate = domain::translate(i18n);
  proposal::decide(
    (&tools, &translate, &services.runners),
    &deciding.scope,
    deciding.seq,
    deciding.decision,
  )
  .await
}
