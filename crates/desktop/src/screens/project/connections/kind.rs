//! Kind of a new agent connection (04.H1 `Byo8K`, option cards `lcwsy`): the two CLIs and the two model
//! APIs Orchester runs agents on itself, two cards a row. A kind this version cannot add is faded with
//! the reason.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::AgentProvider;

use crate::{
  theme::ExtraSpacing,
  ui::{
    field::{FieldNote, field},
    option_card::OptionCard,
  },
};

/// The kinds in the order the cards read.
const KINDS: [AgentProvider; 4] = [
  AgentProvider::CodexCli,
  AgentProvider::ClaudeCodeCli,
  AgentProvider::AnthropicApi,
  AgentProvider::OpenAiApi,
];

/// The *Kind* field: a card per kind, `chosen` selected; choosing another calls `choose`.
pub fn kind_field(ctx: &Ctx, chosen: AgentProvider, choose: &Arc<dyn Fn(AgentProvider) + Send + Sync>) -> Element {
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .overflow_visible()
    .with_children(KINDS.chunks(2).map(|pair| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Step.role())
        .overflow_visible()
        .with_children(pair.iter().map(|&provider| {
          Column::new()
            .flex(1.0)
            .overflow_visible()
            .child(card(ctx, provider, chosen, choose.clone()))
        }))
    }));
  field(&ctx.t("connections.kind.label"), rows.into(), FieldNote::None)
}

fn card(
  ctx: &Ctx,
  provider: AgentProvider,
  chosen: AgentProvider,
  choose: Arc<dyn Fn(AgentProvider) + Send + Sync>,
) -> Element {
  let (id, detail) = match provider {
    AgentProvider::CodexCli => ("connection-kind-codex", "connections.kind.codexDetail"),
    AgentProvider::ClaudeCodeCli => ("connection-kind-claude", "connections.kind.claudeDetail"),
    AgentProvider::AnthropicApi => ("connection-kind-anthropic", "connections.kind.anthropicDetail"),
    AgentProvider::OpenAiApi => ("connection-kind-openai", "connections.kind.openaiDetail"),
  };
  let card = OptionCard::new(
    id,
    ctx.t(super::status::program_key(provider)),
    ctx.t(detail),
    provider == chosen,
  );
  if !provider.can_be_added() {
    return card.unavailable(Some(ctx.t("connections.kind.notYet"))).into();
  }
  card
    .on_choose(move || {
      if provider != chosen {
        choose(provider);
      }
    })
    .into()
}
