//! The card of a run that an archive stopped (`Run state · Stopped · archived` `pLdtd` in A22 `pUu78`, ORC-838), at the
//! top of the Summary: the team archived a resource this run pinned, so it was stopped; the card names the resource,
//! who archived it and when, and the profiles that still name it, which stay not ready (Run again refuses while one
//! does, `docs/design/team-configuration.md`, edge case 8). The worktree, changes and conversation are kept.
//!
//! What still names the resource is read from the team server when the card shows, and only for a kind that keeps a
//! profile from running ([`archive_blocks_profiles`]) while the resource is archived here: restored, or a skill (which
//! this computer cannot tell archived), the card lists the archived resource alone, as it does when the server cannot
//! be asked. A stop recorded before the resource was named says only its name and when.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::SharedResourceKind;
use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};
use orchester_store::{ArchiveStop, Archived};
use orchester_workspace::{display, readiness::archive_blocks_profiles};

use super::{TaskContext, not_ready::rows_box, summary::team_run};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{HubFilter, HubView, ProjectPage, Route},
  services::Services,
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize, NOT_READY_ROW_HEIGHT, TaskRadius, TaskText},
  ui::{
    agent_mark::AgentMark,
    card::{CardSurface, card},
    icon::{Icon, icon},
    pressable::opening,
  },
};

#[derive(Clone, PartialEq)]
pub struct Props {
  task: TaskContext,
  stop: ArchiveStop,
  /// The number of the run the archive stopped.
  run: u32,
}

impl DevtoolsInspectable for Props {}

/// The card for the latest run of `task`, if an archive of a team resource stopped it.
pub fn shown(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let stop = task.detail.archive_stop.clone()?;
  let run = task.detail.run.as_ref()?.run.number().get();
  Some(ctx.mount_keyed::<ArchivedCard>(
    "task-archived-card",
    Props {
      task: task.clone(),
      stop,
      run,
    },
  ))
}

/// What the header's status says of a run the archive stopped (`Status` `ev3VX` in A22 `pUu78`): *Stopped at 03
/// Implement · pencil-design was archived*, or without the step when the run has none to name.
pub fn status_words(ctx: &Ctx, task: &TaskContext) -> Option<Arc<str>> {
  let resource = task.detail.archive_stop.as_ref()?.resource.as_str();
  Some(match stopped_step(ctx, task) {
    Some(step) => ctx.t_args(
      "task.status.stoppedArchivedAt",
      [("step", &*step), ("resource", resource)],
    ),
    None => ctx.t_args("task.status.stoppedArchived", [("resource", resource)]),
  })
}

/// The line under the run's steps (`Last event` `N1C95m`): *03 Implement stopped: the team archived pencil-design,
/// which this run pinned.*
pub fn event_words(ctx: &Ctx, task: &TaskContext) -> Option<Arc<str>> {
  let resource = task.detail.archive_stop.as_ref()?.resource.as_str();
  let (event, no_step) = match task.team {
    Some(_) => ("summary.archived.event", "summary.archived.eventNoStep"),
    None => ("summary.archived.eventLocal", "summary.archived.eventNoStepLocal"),
  };
  Some(match stopped_step(ctx, task) {
    Some(step) => ctx.t_args(event, [("step", &*step), ("resource", resource)]),
    None => ctx.t_args(no_step, [("resource", resource)]),
  })
}

/// The step the stopped run was at, as the run's chips name it: *03 Implement*.
fn stopped_step(ctx: &Ctx, task: &TaskContext) -> Option<Arc<str>> {
  let step = team_run::stopped_in(task.detail.run.as_ref()?)?;
  Some(ctx.t_args(
    "task.status.step",
    [
      ("number", format!("{:02}", step.number)),
      ("step", step.name.to_string()),
    ],
  ))
}

struct ArchivedCard;

impl Component for ArchivedCard {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { task, stop, run } = ctx.props::<Props>().clone();
    let still = still_using(ctx, &task, &stop);
    let navigation = Navigation::of(ctx);
    let project = task.project.open.project.id();
    let resource = stop.resource.as_str();
    let (body, need, glyph) = explain(ctx, &stop, (run, task.team.is_some()));
    let words = |key: &str| {
      let (run, at) = (run.to_string(), display::clock(stop.stopped_at));
      ctx.t_args(
        key,
        [("resource", resource), ("run", run.as_str()), ("time", at.as_str())],
      )
    };
    let mut rows = vec![row(
      "task-archived-resource",
      (icon(glyph, IconSize::Md, PaletteColor::Danger).into(), still.is_empty()),
      Text::new(resource).variant(CardsText::Decided.role()),
      Some(
        Text::new(&need)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Danger),
      ),
      None,
    )];
    for (index, profile) in still.iter().enumerate() {
      let open = navigation.to(Route::Project(
        project,
        ProjectPage::hub(HubView::new(HubFilter::Profiles, Some(profile.id))),
      ));
      rows.push(profile_row(
        ctx,
        (index, profile),
        (resource, index + 1 == still.len()),
        open,
      ));
    }
    card("task-archived", CardSurface::Stopped)
      .rounded(TaskRadius::NotReadyCard.role())
      .spacing(SpacingSize::Md)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::Center)
          .spacing(SpacingSize::Sm)
          .child(icon(Icon::Archive, IconSize::Md, PaletteColor::Danger))
          .child(
            Text::new(&words("summary.archived.title"))
              .id("task-archived-title")
              .variant(TaskText::SectionTitle.role()),
          )
          .child(
            Text::new(&words("summary.archived.meta"))
              .id("task-archived-meta")
              .variant(TypographyStyle::Description),
          ),
      )
      .child(
        Text::new(&body)
          .id("task-archived-body")
          .variant(ExtraTypography::Lead.role()),
      )
      .child(rows_box(rows))
  }
}

/// What the card says of the stop: its body, what the first row says of the resource, and that row's glyph. A stop
/// recorded before the confirmation named the resource says only its name and when. In a local project the person
/// archived it, not a team: it says *you*, and names no server's person.
fn explain(ctx: &Ctx, stop: &ArchiveStop, (run, team): (u32, bool)) -> (Arc<str>, Arc<str>, Icon) {
  let at = display::clock(stop.stopped_at);
  let resource = stop.resource.as_str();
  let words = |key: &str, extra: &[(&str, &str)]| {
    let run = run.to_string();
    let mut args = vec![("resource", resource), ("run", run.as_str()), ("time", at.as_str())];
    args.extend_from_slice(extra);
    ctx.t_args(key, args)
  };
  match &stop.archived {
    Some(archived) => {
      let kind = ctx.t(kind_key(archived.kind));
      let (body, need) = match (team, archived.by.as_deref()) {
        (false, _) => (
          words("summary.archived.bodyLocal", &[("kind", &kind)]),
          words("summary.archived.archivedLocal", &[("kind", &kind)]),
        ),
        (true, Some(who)) => (
          words("summary.archived.body", &[("who", who), ("kind", &kind)]),
          words("summary.archived.archivedBy", &[("who", who), ("kind", &kind)]),
        ),
        (true, None) => (
          words("summary.archived.bodyUnnamed", &[("kind", &kind)]),
          words("summary.archived.archivedUnnamed", &[("kind", &kind)]),
        ),
      };
      (body, need, kind_glyph(archived.kind))
    }
    None => (
      words("summary.archived.bodyLegacy", &[]),
      words("summary.archived.archivedLegacy", &[]),
      Icon::Archive,
    ),
  }
}

/// A profile that still names the archived resource, at its latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Profile {
  id: ResourceId,
  name: String,
  version: VersionNumber,
  /// The CLI it runs, whose mark leads its row; none when this computer cannot say.
  cli: Option<AgentProvider>,
}
impl DevtoolsInspectable for Profile {}

/// The profiles whose latest version still names the archived resource, as the team server reads them now; none when
/// the stop does not say which resource it was, when the resource no longer keeps a profile from running (restored
/// here, or a kind this computer cannot tell archived), or when the server cannot be asked.
fn still_using(ctx: &mut Ctx, task: &TaskContext, stop: &ArchiveStop) -> Vec<Profile> {
  let services = Services::of(ctx);
  let archived = stop
    .archived
    .as_ref()
    .filter(|archived| task.detail.archived_still && archive_blocks_profiles(archived.kind))
    .map(|archived: &Archived| archived.id);
  let state = ctx
    .future(
      (task.project.open.repo_root.clone(), archived),
      move |(root, archived)| {
        let services = services.clone();
        async move {
          let Some(archived) = archived else {
            return Ok(Vec::new());
          };
          let used = services.servers.used_by(&root, archived).await.map_err(|_| ())?;
          let mut profiles = Vec::new();
          for used in used
            .resources
            .into_iter()
            .filter(|used| used.kind == SharedResourceKind::AgentProfile)
          {
            profiles.push(Profile {
              cli: services.workspace.team_profile_cli(&root, used.id),
              id: used.id,
              name: used.name,
              version: used.version,
            });
          }
          Ok::<_, ()>(profiles)
        }
      },
    )
    .state()
    .get();
  state.data.unwrap_or_default()
}

/// A profile that still names the resource: its name and version, why it cannot run, and where to change it.
fn profile_row(ctx: &Ctx, (index, profile): (usize, &Profile), (resource, last): (&str, bool), open: Open) -> Element {
  let id = format!("task-archived-profile-{index}");
  let name = ctx.t_args(
    "summary.archived.profile",
    [
      ("name", profile.name.as_str()),
      ("version", &profile.version.to_string()),
    ],
  );
  let need = ctx.t_args("summary.archived.stillUses", [("resource", resource)]);
  let link = opening(open)
    .id(format!("{id}-open"))
    .child(Text::new(&ctx.t("summary.archived.open")).variant(TypographyStyle::Button));
  let glyph: Element = match profile.cli {
    Some(cli) => AgentMark::of(cli).render(ctx, IconSize::Md),
    None => icon(Icon::Bot, IconSize::Md, PaletteColor::TextSecondary).into(),
  };
  row(
    &id,
    (glyph, last),
    Text::new(&name).variant(CardsText::Decided.role()),
    Some(Text::new(&need).variant(TypographyStyle::Description)),
    Some(link.into()),
  )
}

/// One row of the list: its `glyph`, what it is about, what is the matter with it and, at the end, where to look; every
/// row but the `last` has a hairline under it.
fn row(id: &str, (glyph, last): (Element, bool), what: Text, need: Option<Text>, link: Option<Element>) -> Element {
  let mut row = Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(NOT_READY_ROW_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Loose.role())
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Snug.role()))
    .child(glyph)
    .child(what.id(format!("{id}-what")));
  if !last {
    row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  if let Some(need) = need {
    row = row.child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(need.id(format!("{id}-need"))),
    );
  }
  row.with_children(link).into()
}

/// What the resource that stopped the run is called.
const fn kind_key(kind: ResourceKind) -> &'static str {
  match kind {
    ResourceKind::AgentProfile => "summary.archived.kind.profile",
    ResourceKind::Pipeline => "summary.archived.kind.pipeline",
    ResourceKind::Instruction => "summary.archived.kind.instruction",
    ResourceKind::Skill => "summary.archived.kind.skill",
    ResourceKind::Tool => "summary.archived.kind.tool",
    ResourceKind::McpServer => "summary.archived.kind.mcp",
    ResourceKind::TeamReviewer => "summary.archived.kind.reviewer",
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => "summary.archived.kind.other",
  }
}

/// The glyph a list draws for what `kind` is: the hub's (`Item` in `t4mtQL`), a plug for an MCP server.
const fn kind_glyph(kind: ResourceKind) -> Icon {
  match kind {
    ResourceKind::McpServer => Icon::Plug,
    ResourceKind::Tool => Icon::Wrench,
    ResourceKind::Instruction => Icon::FileText,
    ResourceKind::Skill => Icon::Sparkles,
    ResourceKind::Pipeline => Icon::Workflow,
    ResourceKind::AgentProfile | ResourceKind::TeamReviewer => Icon::Bot,
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => Icon::Archive,
  }
}
