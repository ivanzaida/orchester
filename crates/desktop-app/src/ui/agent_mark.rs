//! The marks of the agents' makers (`Agent marks` `z7PmEL` on Components `G7K9ux`): where a screen shows a CLI or an
//! API by its kind, it shows its maker's own mark instead of a lucide glyph. Claude Code is the Claude mark
//! (`Mark/Claude` `eFnry`) in `mark_claude`; Codex and OpenAI's API the OpenAI knot (`Mark/OpenAI` `e2P9hh`) and
//! Anthropic's API Anthropic's "A" (`Mark/Anthropic` `OiwFt`), both in `text_primary`. The marks are Simple Icons'
//! SVGs (CC0), filled in a theme color, as large as an icon of `size`.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};
use orchester_core::connection::AgentProvider;

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
  /// The mark of an agent of `provider`.
  pub const fn of(provider: AgentProvider) -> Self {
    match provider {
      AgentProvider::ClaudeCodeCli => Self::Claude,
      AgentProvider::CodexCli | AgentProvider::OpenAiApi => Self::OpenAi,
      AgentProvider::AnthropicApi => Self::Anthropic,
    }
  }

  /// The color the design fills the mark with.
  pub fn color(self) -> PaletteColor {
    match self {
      Self::Claude => ExtraColor::MarkClaude.role(),
      Self::OpenAi | Self::Anthropic => PaletteColor::TextPrimary,
    }
  }

  const fn svg(self) -> &'static str {
    match self {
      Self::Claude => CLAUDE_SVG,
      Self::OpenAi => OPENAI_SVG,
      Self::Anthropic => ANTHROPIC_SVG,
    }
  }
}

/// The mark of an agent of `provider`, in its color, as large as an icon of `size`.
pub fn agent_mark(ctx: &Ctx, provider: AgentProvider, size: IconSize) -> Element {
  let mark = AgentMark::of(provider);
  let fill = ctx.theme().palette_color(mark.color());
  let side = Dimension::Px(size.size());
  Svg::new(SvgData::from_str(mark.svg()).with_fill(fill))
    .describe("mark", format!("{mark:?}"))
    .size(side, side)
    .into()
}
