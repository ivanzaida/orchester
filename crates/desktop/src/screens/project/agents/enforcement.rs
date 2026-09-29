//! *Enforced by <CLI>* (`kXyN7` in 04.04 `T1rjv`): each setting with how the chosen connection's
//! CLI enforces it, or that it cannot. The design has it come from a capability check of the CLI;
//! what exists is core's capability table for this platform, so the panel says whose known limits
//! it shows rather than a check time, then what it enforces for the chosen MCP servers (`mcp`). Then the rules: no
//! bypass mode, steps only narrow.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  agent_files::AgentFilesMechanism,
  connection::{AgentConnection, AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, NetworkAccess,
    capability::{EnforcedPermissions, PermissionOption, PermissionProblem, PermissionSetting, support},
  },
};

use super::editor::ProfileEditor;
use crate::{
  agents::permissions::{mechanism, option_title, setting_label},
  screens::project::connections::status::program_key,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// Panel width (`kXyN7`).
const PANEL_WIDTH: f32 = 300.0;
const DIVIDER_HEIGHT: f32 = 1.0;

impl ProfileEditor {
  pub(super) fn enforcement_panel(&self, ctx: &Ctx) -> Element {
    let panel = Column::new()
      .id("agent-enforcement")
      .width(Dimension::Px(PANEL_WIDTH))
      .padding(ExtraSpacing::Step.role())
      .spacing(ExtraSpacing::Group.role())
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(
        lurq::app::theme::BorderSize::Sm,
        ExtraColor::Hairline.role(),
      ))
      .rounded(RadiusSize::Lg);
    let rows = match (self.selected_connection(), self.enforcement()) {
      (Some(connection), Some(result)) => {
        let mut rows = vec![heading(ctx, connection)];
        rows.extend(setting_rows(
          ctx,
          connection.provider(),
          &self.permissions.get(),
          &result,
        ));
        rows.extend(self.agent_files_rows(ctx, connection.provider()));
        rows.extend(self.mcp_enforcement(ctx, connection.provider()));
        rows
      }
      _ => vec![line(Icon::Plug, &ctx.t("agents.permissions.enforcement.noConnection"))],
    };
    panel
      .with_children(rows)
      .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role()))
      .child(line(Icon::Shield, &ctx.t("agents.permissions.enforcement.noBypass")))
      .child(line(
        Icon::Workflow,
        &ctx.t("agents.permissions.enforcement.stepsNarrow"),
      ))
      .into()
  }
}

impl ProfileEditor {
  /// Whether the CLI loads the repository's own agent files and how (`e9t1b` in `bYDCI`), and
  /// that the project sets it, not the profile (`k9Q3LT`).
  fn agent_files_rows(&self, ctx: &Ctx, provider: AgentProvider) -> [Element; 2] {
    let setting = self.context.agents.agent_files;
    let (glyph, title) = if setting.is_on() {
      (Icon::FolderGit, "agents.permissions.enforcement.repositoryFiles.on")
    } else {
      (Icon::CircleCheck, "agents.permissions.enforcement.repositoryFiles.off")
    };
    let how = ctx.t(match setting.mechanism(provider) {
      AgentFilesMechanism::CodexLoads => "agents.permissions.enforcement.repositoryFiles.codexLoads",
      AgentFilesMechanism::CodexLeftOut => "agents.permissions.enforcement.repositoryFiles.codexLeftOut",
      AgentFilesMechanism::ClaudeNoProjectSettings => "agents.permissions.enforcement.repositoryFiles.claude",
    });
    let project = self.context.project.project.name().as_str();
    [
      row(glyph, &ctx.t(title), &how),
      line(
        Icon::FolderGit,
        &ctx.t_args(
          "agents.permissions.enforcement.repositoryFiles.note",
          [("project", project)],
        ),
      ),
    ]
  }
}

/// "Enforced by Codex CLI" and whose limits these are (`FLmZD`).
fn heading(ctx: &Ctx, connection: &AgentConnection) -> Element {
  let cli = ctx.t(program_key(connection.provider()));
  let platform = ctx.t(match Platform::current() {
    Platform::Windows => "agents.permissions.enforcement.platform.windows",
    Platform::MacOs => "agents.permissions.enforcement.platform.macos",
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&ctx.t_args("agents.permissions.enforcement.title", [("cli", &*cli)])).variant(TypographyStyle::Label),
    )
    .child(
      Text::new(&ctx.t_args(
        "agents.permissions.enforcement.source",
        [("connection", connection.name().as_str()), ("platform", &*platform)],
      ))
      .variant(TypographyStyle::Caption),
    )
    .into()
}

/// One row per setting (`F5JRp`, `FSFlp`, `A9ogd`), and one for the extra writable folders (not drawn). Network reads
/// *stays on* wherever the connection cannot enforce off, whatever was chosen, as the locked switch beside it shows.
fn setting_rows(
  ctx: &Ctx,
  provider: AgentProvider,
  permissions: &AgentPermissions,
  result: &Result<EnforcedPermissions, PermissionProblem>,
) -> Vec<Element> {
  let enforced = match result {
    Ok(enforced) => enforced,
    Err(problem) => return vec![not_enforceable(ctx, problem)],
  };
  let commands = PermissionOption::of_commands(permissions.commands());
  let network_stays_on = !support(provider, Platform::current(), PermissionOption::NetworkOff).is_enforced();
  let rows = [
    (
      PermissionSetting::Repository,
      ctx.t(setting_label(PermissionSetting::Repository)),
    ),
    (PermissionSetting::Commands, ctx.t(option_title(commands))),
    (
      PermissionSetting::Network,
      ctx.t(match permissions.network() {
        NetworkAccess::Off => "agents.permissions.long.networkOff",
        NetworkAccess::On => "agents.permissions.long.networkOn",
      }),
    ),
  ];
  let mut rows: Vec<Element> = rows
    .into_iter()
    .map(|(setting, name)| {
      let how = mechanism(ctx, enforced.mechanism(setting), enforced.permissions());
      let (glyph, title) = if setting == PermissionSetting::Network && network_stays_on {
        (Icon::Lock, ctx.t("agents.permissions.enforcement.staysOn"))
      } else {
        (
          Icon::CircleCheck,
          ctx.t_args("agents.permissions.enforcement.enforced", [("setting", &*name)]),
        )
      };
      row(glyph, &title, &how)
    })
    .collect();
  // Extra writable folders, after repository access as on the tab, when the agent gets any.
  if let Some(folders) = enforced.writable_folders() {
    let name = ctx.t("agents.permissions.folders");
    let title = ctx.t_args("agents.permissions.enforcement.enforced", [("setting", &*name)]);
    rows.insert(
      1,
      row(
        Icon::CircleCheck,
        &title,
        &mechanism(ctx, folders, enforced.permissions()),
      ),
    );
  }
  rows
}

fn not_enforceable(ctx: &Ctx, problem: &PermissionProblem) -> Element {
  let name = match problem {
    PermissionProblem::NoEnforceableOption { setting } => ctx.t(setting_label(*setting)),
    PermissionProblem::NotEnforceable { option, .. } => ctx.t(option_title(*option)),
  };
  let title = ctx.t_args("agents.permissions.enforcement.notEnforceable", [("setting", &*name)]);
  line(Icon::Lock, &title)
}

/// A setting and how it is enforced (`S9nIp1`): the text over the mechanism in mono.
pub(super) fn row(glyph: Icon, title: &str, how: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextPrimary))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .child(Text::new(title).variant(ExtraTypography::EnforcementSetting.role()))
        .child(Text::new(how).variant(ExtraTypography::EnforcementMechanism.role())),
    )
    .into()
}

/// A rule of the panel (`NvU6w`, `B0BVa`): a muted icon and its text.
fn line(glyph: Icon, text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextSecondary))
    .child(Text::new(text).variant(ExtraTypography::Detail.role()).flex(1.0))
    .into()
}
