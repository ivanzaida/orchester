//! The marks of the makers whose agents Orchester runs (`Agent marks` `z7PmEL` on Components `G7K9ux`): Claude Code's
//! Claude mark in its orange (`Mark/Claude` `eFnry`), the OpenAI knot of Codex and the OpenAI API (`Mark/OpenAI`
//! `e2P9hh`) and Anthropic's "A" of the Anthropic API (`Mark/Anthropic` `OiwFt`), both in `text_primary`. The SVGs are
//! Simple Icons' (CC0), drawn as large as an icon of the size asked for. A CLI on an API provider such as DeepSeek runs
//! on someone else's models, so it shows no maker's mark ([`AgentMark::of_connection`]); Lucide stays for the rest.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};
use orchester_core::connection::{AgentConnection, AgentProvider};

use crate::theme::{ExtraColor, IconSize};

const CLAUDE_SVG: &str = include_str!("../../assets/icons/claude.svg");
const OPENAI_SVG: &str = include_str!("../../assets/icons/openai.svg");
const ANTHROPIC_SVG: &str = include_str!("../../assets/icons/anthropic.svg");

/// A maker's mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentMark {
  Claude,
  OpenAi,
  Anthropic,
}

impl AgentMark {
  /// The mark of the agents `provider` runs: Claude Code's Claude, Codex's and the OpenAI API's OpenAI, the Anthropic
  /// API's Anthropic.
  pub const fn of(provider: AgentProvider) -> Self {
    match provider {
      AgentProvider::ClaudeCodeCli => Self::Claude,
      AgentProvider::CodexCli | AgentProvider::OpenAiApi => Self::OpenAi,
      AgentProvider::AnthropicApi => Self::Anthropic,
    }
  }

  /// The mark of `connection`, none for a CLI on an API provider.
  pub fn of_connection(connection: &AgentConnection) -> Option<Self> {
    connection
      .api_provider()
      .is_none()
      .then(|| Self::of(connection.provider()))
  }

  const fn svg(self) -> &'static str {
    match self {
      Self::Claude => CLAUDE_SVG,
      Self::OpenAi => OPENAI_SVG,
      Self::Anthropic => ANTHROPIC_SVG,
    }
  }

  /// The role the mark is filled with.
  pub fn color(self) -> PaletteColor {
    match self {
      Self::Claude => ExtraColor::MarkClaude.role(),
      Self::OpenAi | Self::Anthropic => PaletteColor::TextPrimary,
    }
  }

  /// The mark, as large as an icon of `size`.
  pub fn render(self, ctx: &Ctx, size: IconSize) -> Element {
    let fill = ctx.theme().palette_color(self.color());
    let side = Dimension::Px(size.size());
    Svg::new(SvgData::from_str(self.svg()).with_fill(fill))
      .size(side, side)
      .into()
  }
}
