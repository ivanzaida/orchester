//! Kind of a new agent connection (04.H1 `Byo8K`, compact option cards `lcwsy` in `W897Q`): the two CLIs
//! and the two model APIs Orchester runs agents on itself, in one row, with what native kinds are
//! (`nFBxV`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::AgentProvider;

use crate::{
  theme::{ExtraSpacing, ExtraTypography},
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

/// The *Kind* field: a compact card per kind in one row, `chosen` selected, and what native kinds
/// are; choosing another calls `choose`.
pub fn kind_field(ctx: &Ctx, chosen: AgentProvider, choose: &Arc<dyn Fn(AgentProvider) + Send + Sync>) -> Element {
  let cards = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Stretch)
    .overflow_visible()
    .with_children(KINDS.iter().map(|&provider| {
      Column::new()
        .flex(1.0)
        .overflow_visible()
        .child(card(ctx, provider, chosen, choose.clone()))
    }));
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .overflow_visible()
    .child(cards)
    .child(Text::new(&ctx.t("connections.kind.nativeHint")).variant(ExtraTypography::Hint.role()));
  field(&ctx.t("connections.kind.label"), content.into(), FieldNote::None)
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
  OptionCard::new(
    id,
    ctx.t(super::status::program_key(provider)),
    ctx.t(detail),
    provider == chosen,
  )
  .compact()
  .on_choose(move || {
    if provider != chosen {
      choose(provider);
    }
  })
  .into()
}
