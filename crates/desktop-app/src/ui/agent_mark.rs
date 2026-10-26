//! The makers' marks of the agent CLIs and model APIs (`Mark/Claude` `eFnry`, `Mark/OpenAI` `e2P9hh`,
//! `Mark/Anthropic` `OiwFt` in the new `orchester.pen`, Components section `z7PmEL`): Claude Code shows Claude's mark
//! in `mark_claude`, Codex and the OpenAI API OpenAI's and the Anthropic API Anthropic's, both in `text_primary`. The
//! marks are the makers' own SVGs (`assets/icons/`), filled in a theme color, as large as the caller asks.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};
use orchester_core::{account::AccountCli, connection::AgentProvider};

use crate::theme::ExtraColor;

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
  /// The mark of an account's CLI: Claude Code's is Claude's, Codex's OpenAI's.
  pub const fn of_cli(cli: AccountCli) -> Self {
    match cli {
      AccountCli::ClaudeCode => Self::Claude,
      AccountCli::Codex => Self::OpenAi,
    }
  }

  /// The mark of an agent provider: a CLI's as [`Self::of_cli`], a model API's its maker's.
  pub const fn of_provider(provider: AgentProvider) -> Self {
    match provider {
      AgentProvider::ClaudeCodeCli => Self::Claude,
      AgentProvider::CodexCli | AgentProvider::OpenAiApi => Self::OpenAi,
      AgentProvider::AnthropicApi => Self::Anthropic,
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

  const fn svg(self) -> &'static str {
    match self {
      Self::Claude => CLAUDE_SVG,
      Self::OpenAi => OPENAI_SVG,
      Self::Anthropic => ANTHROPIC_SVG,
    }
  }

  fn color(self) -> PaletteColor {
    match self {
      Self::Claude => ExtraColor::MarkClaude.role(),
      Self::OpenAi | Self::Anthropic => PaletteColor::TextPrimary,
    }
  }
}

/// `mark`, `side` wide and high, in its color.
pub fn agent_mark(ctx: &Ctx, mark: AgentMark, side: f32) -> Element {
  let fill = ctx.theme().palette_color(mark.color());
  let side = Dimension::Px(side);
  Svg::new(SvgData::from_str(mark.svg()).with_fill(fill))
    .size(side, side)
    .into()
}
