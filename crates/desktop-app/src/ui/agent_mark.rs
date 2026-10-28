//! The marks of the agents' makers (`Agent marks` `z7PmEL` on Components `G7K9ux`): where a screen shows a CLI or an
//! API by its kind, it shows its maker's own mark instead of a lucide glyph. Claude Code is the Claude mark
//! (`Mark/Claude` `eFnry`) in `mark_claude`; Codex and OpenAI's API the OpenAI knot (`Mark/OpenAI` `e2P9hh`) and
//! Anthropic's API Anthropic's "A" (`Mark/Anthropic` `OiwFt`), both in `text_primary`. The marks are Simple Icons'
//! SVGs (CC0), filled in a theme color, as large as an icon of the size the caller asks. A CLI on
//! an API provider such as DeepSeek runs on someone else's models, so it shows no maker's mark
//! ([`AgentMark::of_connection`]). One helper serves the Orchestrator's runs-on picker, Connections and Settings.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};
use orchester_core::{
  account::AccountCli,
  connection::{AgentConnection, AgentProvider},
};

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

  /// The mark in its color, as large as an icon of `size`.
  pub fn render(self, ctx: &Ctx, size: IconSize) -> Element {
    let fill = ctx.theme().palette_color(self.color());
    let side = Dimension::Px(size.size());
    Svg::new(SvgData::from_str(self.svg()).with_fill(fill))
      .describe("mark", format!("{self:?}"))
      .size(side, side)
      .into()
  }
}
