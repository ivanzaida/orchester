//! The marks of the agents' makers (`Agent marks` `z7PmEL` on Components `G7K9ux`): where a screen shows a CLI or an
//! API by its kind, it shows its maker's own mark instead of a lucide glyph. Claude Code is the Claude mark
//! (`Mark/Claude` `eFnry`) in `mark_claude`; Codex and OpenAI's API the OpenAI knot (`Mark/OpenAI` `e2P9hh`) and
//! Anthropic's API Anthropic's "A" (`Mark/Anthropic` `OiwFt`), both in `text_primary`. The marks are Simple Icons'
//! SVGs (CC0), filled in a theme color, as large as an icon of `size` or as the caller asks.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};
use orchester_core::{account::AccountCli, connection::AgentProvider};

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

  /// The mark of an account's CLI: Claude Code's is Claude's, Codex's OpenAI's.
  pub const fn of_cli(cli: AccountCli) -> Self {
    match cli {
      AccountCli::ClaudeCode => Self::Claude,
      AccountCli::Codex => Self::OpenAi,
    }
  }

  /// The mark of an MCP client by the name it gave itself (`claude-code`, `codex-mcp-client`), when the name is one
  /// of the CLIs'.
  pub fn of_client(name: &str) -> Option<Self> {
    let name = name.to_ascii_lowercase();
    if name.contains("claude") {
      Some(Self::Claude)
    } else if name.contains("codex") {
      Some(Self::OpenAi)
    } else {
      None
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
  mark_icon(ctx, AgentMark::of(provider), size.size())
}

/// `mark`, `side` wide and high, in its color.
pub fn mark_icon(ctx: &Ctx, mark: AgentMark, side: f32) -> Element {
  let fill = ctx.theme().palette_color(mark.color());
  let side = Dimension::Px(side);
  Svg::new(SvgData::from_str(mark.svg()).with_fill(fill))
    .describe("mark", format!("{mark:?}"))
    .size(side, side)
    .into()
}
