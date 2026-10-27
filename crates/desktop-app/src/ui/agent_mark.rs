//! The mark of who runs an agent (`Claude` and `Codex` in the cards of `Tasks board · redesign` `J6gQo`): Claude's in
//! `mark_claude` for a Claude agent (Claude Code, or Anthropic's API run by Orchester), the OpenAI knot in `text_primary`
//! for a Codex or OpenAI agent, 14 square. The bundled lucide font carries no brand icons, so both are the design's
//! own paths (`assets/icons/claude.svg`, `codex.svg`).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};
use orchester_core::connection::AgentProvider;

use crate::theme::ExtraColor;

const CLAUDE_SVG: &str = include_str!("../../assets/icons/claude.svg");
const CODEX_SVG: &str = include_str!("../../assets/icons/codex.svg");
/// The mark's side (`Claude` `o7C1a`).
const SIDE: f32 = 14.0;

/// The mark of `provider`, its element id `id` and its provider's name for tools.
pub fn agent_mark(ctx: &Ctx, id: &str, provider: AgentProvider) -> Element {
  let (svg, color, name) = match provider {
    AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi => (CLAUDE_SVG, ExtraColor::MarkClaude.role(), "claude"),
    AgentProvider::CodexCli | AgentProvider::OpenAiApi => (CODEX_SVG, PaletteColor::TextPrimary, "codex"),
  };
  let fill = ctx.theme().palette_color(color);
  Svg::new(SvgData::from_str(svg).with_fill(fill))
    .id(id.to_owned())
    .describe("agent", name)
    .size(Dimension::Px(SIDE), Dimension::Px(SIDE))
    .into()
}
