//! The Kind group of Add provider (`Kind` `rSMnI`): the four kinds side by side as compact cards (`Cards` `Q4AgH`),
//! each led by its maker's mark with one short line under its name. The OpenAI API's native kind is locked, why in its
//! line and its reason, until the runner runs its agents ([`AgentProvider::runs_agents`]). Choosing another kind
//! suggests its name, unless the person typed one.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::AgentProvider;
use orchester_workspace::connections::data::ProjectConnections;

use super::{
  AddAgent, Kind,
  naming::{suggest, suggested_name},
};
use crate::{
  screens::project_settings::kit::Group,
  theme::IconSize,
  ui::{agent_mark::AgentMark, icon::Icon, option_card::OptionCard},
};

/// Each kind: its card's id, its glyph, its name and its line.
const KINDS: [(AgentProvider, &str, Icon, &str, &str); 4] = [
  (
    AgentProvider::ClaudeCodeCli,
    "connection-kind-claude",
    Icon::Terminal,
    "connections.provider.claude",
    "connections.kind.claudeShort",
  ),
  (
    AgentProvider::CodexCli,
    "connection-kind-codex",
    Icon::Terminal,
    "connections.provider.codex",
    "connections.kind.codexShort",
  ),
  (
    AgentProvider::AnthropicApi,
    "connection-kind-anthropic",
    Icon::Cpu,
    "connections.provider.anthropicApiShort",
    "connections.kind.anthropicShort",
  ),
  (
    AgentProvider::OpenAiApi,
    "connection-kind-openai",
    Icon::Cpu,
    "connections.provider.openaiApiShort",
    "connections.kind.nativeShort",
  ),
];

impl AddAgent {
  pub(super) fn kind_section(&self, ctx: &Ctx, listed: Option<&ProjectConnections>, project: &str) -> Element {
    let cards = Row::new()
      .id("connection-kinds")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(KINDS.into_iter().map(|(provider, id, glyph, title, line)| {
        Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(self.card(
          ctx,
          (listed, project),
          provider,
          (id, glyph, title, line),
        ))
      }));
    Group::new("connection-kind-section", ctx.t("connections.kind.label"))
      .after(cards)
      .into()
  }

  fn card(
    &self,
    ctx: &Ctx,
    (listed, project): (Option<&ProjectConnections>, &str),
    provider: AgentProvider,
    (id, glyph, title, line): (&str, Icon, &str, &str),
  ) -> Element {
    let (kind, program) = (self.kind.clone(), self.program.clone());
    let (name, suggested) = (self.name.clone(), self.suggested.clone());
    let suggestion = listed.map(|listed| suggested_name(ctx.i18n(), listed, provider, project));
    let line = match provider.program_name() {
      Some(program) => ctx.t_args(line, [("program", program)]),
      None => ctx.t(line),
    };
    let card = OptionCard::new(
      id.to_owned(),
      (glyph, ctx.t(title)),
      line,
      self.kind.get().0 == provider,
      move || {
        program.set(String::new());
        kind.set(Kind(provider));
        if let Some(new) = suggestion.clone() {
          suggest(&name, &suggested, new);
        }
      },
    )
    .compact()
    .mark(AgentMark::of(provider).render(ctx, IconSize::Md));
    match provider.runs_agents() {
      true => card.into(),
      false => card.unavailable(ctx.t("connections.kind.openaiUnavailable")).into(),
    }
  }
}
