//! The Tools tab's list (`04 Tools` `c05kx`): a row per tool with its kind, command and secrets, the steps that call
//! it and its version, then who owns the tools and which kinds are not available yet. With no tool, a row says what
//! a command tool is (06b).
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, resource::ToolAction};
use orchester_workspace::{
  agents::data::VersionState,
  tools::data::{ProjectTools, ToolEntry},
};

use super::words;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, ToolTab},
  screens::hub::{
    self,
    parts::{Entry, Tone, boxed, group, head, note, quiet, status},
  },
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
  },
};

pub(super) fn list(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectTools) -> Element {
  let mut pane = Column::new()
    .id("tools-library")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Sections.role());
  let (id, nav) = (project.open.project.id(), Navigation::of(ctx));
  let rows: Vec<Element> = if listed.tools.is_empty() {
    vec![empty(ctx, &nav, id)]
  } else {
    listed
      .tools
      .iter()
      .map(|entry| row(ctx, (&nav, id), listed, entry))
      .collect()
  };
  let count = listed.tools.len().to_string();
  pane = pane
    .child(group(
      "tools-list",
      head(
        "tools-list",
        &ctx.t("hub.tabs.tools"),
        (!listed.tools.is_empty()).then_some(&*count),
        None,
      ),
      boxed("tools-rows", rows),
    ))
    .child(note(
      "tools-ownership",
      Icon::Info,
      &ctx.t_args("tools.ownership", [("project", project.open.project.name().as_str())]),
    ))
    .child(note("tools-other-kinds", Icon::Lock, &ctx.t("tools.otherKinds")));
  hub::list_body("tools-list-page", pane)
}

fn row(ctx: &Ctx, (nav, project): (&Navigation, ProjectId), listed: &ProjectTools, entry: &ToolEntry) -> Element {
  let (kind, glyph) = words::kind(&entry.tool);
  let detail = match entry.tool.action() {
    ToolAction::Command(command) => command.command_line(),
    _ => entry.tool.description().to_owned(),
  };
  let secrets = ctx.t(if entry.tool.access().is_some() {
    "tools.needsSecret"
  } else {
    "tools.noSecrets"
  });
  let hint = ctx.t_args(
    "tools.rowLine",
    [
      ("kind", ctx.t(kind).to_string()),
      ("detail", detail),
      ("secrets", secrets.to_string()),
    ],
  );
  let used = super::words::used(ctx, listed.used_by(entry.id).count());
  let (tone, standing) = if !entry.runnable() {
    (Tone::Muted, ctx.t("tools.unavailable"))
  } else {
    match entry.state {
      VersionState::Published(_) => (Tone::Success, ctx.t("tools.ready")),
      VersionState::Draft { published: None } => (Tone::Muted, ctx.t("tools.draft")),
      _ => (Tone::Warning, ctx.t("tools.draft")),
    }
  };
  let version = ctx.t_args(
    "tools.rowStatus",
    [
      ("status", standing.to_string()),
      ("version", words::version(ctx, entry).to_string()),
    ],
  );
  let open = nav.to(Route::Project(
    project,
    ProjectPage::Tool(entry.id, ToolTab::Configuration),
  ));
  Entry::new(format!("tool-row-{}", entry.id), entry.name.clone())
    .tile(glyph)
    .hint(hint)
    .end(quiet(&used))
    .end(status(&format!("tool-status-{}", entry.id), tone, &version))
    .press(Press::Open(open))
    .into()
}

/// No tool yet (06b): what a command tool does, with Add command tool.
fn empty(ctx: &Ctx, nav: &Navigation, project: ProjectId) -> Element {
  let add = ActionButton::new("tools-create-command", ButtonKind::Secondary, ctx.t("tools.addCommand"))
    .icon(Icon::Terminal)
    .opens(nav.to(Route::Project(project, ProjectPage::NewTool)));
  Entry::new("tools-empty", ctx.t("tools.emptyTitle"))
    .tile(Icon::Wrench)
    .hint(ctx.t("tools.emptyDetail"))
    .end(add)
    .into()
}
