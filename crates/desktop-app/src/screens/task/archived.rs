//! The card of a run that an archive stopped (`Run state · Stopped · archived` `pLdtd` in A22 `pUu78`, ORC-838), at the
//! top of the Summary: the team archived a resource this run pinned, so it was stopped; the card names the resource,
//! who archived it and when, and the profiles that still name it, which stay not ready (Run again refuses while one
//! does, `docs/design/team-configuration.md`, edge case 8). The worktree, changes and conversation are kept.
//!
//! What still names the resource is read from the team server when the card shows; when it cannot be read the card
//! lists the archived resource alone.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};
use orchester_store::{ArchiveStop, Archived};
use orchester_workspace::display;

use super::{TaskContext, not_ready::rows_box};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{HubFilter, HubView, ProjectPage, Route},
  services::Services,
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize, NOT_READY_ROW_HEIGHT, TaskRadius, TaskText},
  ui::{
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

/// What the status line says after the status: *pencil-design was archived*.
pub fn status_reason(ctx: &Ctx, task: &TaskContext) -> Option<Arc<str>> {
  let stop = task.detail.archive_stop.as_ref()?;
  Some(ctx.t_args("task.status.archived", [("resource", stop.resource.as_str())]))
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
    let at = display::clock(stop.stopped_at);
    let resource = stop.resource.as_str();
    let kind = stop.archived.as_ref().map(|archived| ctx.t(kind_key(archived.kind)));
    let words = |key: &str, extra: &[(&str, &str)]| {
      let run = run.to_string();
      let mut args = vec![("resource", resource), ("run", run.as_str()), ("time", at.as_str())];
      args.extend_from_slice(extra);
      ctx.t_args(key, args)
    };
    let who = stop.archived.as_ref().and_then(|archived| archived.by.as_deref());
    let kind_words = kind.as_deref().unwrap_or("");
    let body = match who {
      Some(who) => words("summary.archived.body", &[("who", who), ("kind", kind_words)]),
      None => words("summary.archived.bodyUnnamed", &[("kind", kind_words)]),
    };
    let need = match who {
      Some(who) => words("summary.archived.archivedBy", &[("who", who), ("kind", kind_words)]),
      None => words("summary.archived.archivedUnnamed", &[("kind", kind_words)]),
    };
    let mut rows = vec![row(
      "task-archived-resource",
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
      rows.push(profile_row(ctx, (index, profile), resource, open));
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
            Text::new(&words("summary.archived.title", &[]))
              .id("task-archived-title")
              .variant(TaskText::SectionTitle.role()),
          )
          .child(
            Text::new(&words("summary.archived.meta", &[]))
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

/// A profile that still names the archived resource, at its latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Profile {
  id: ResourceId,
  name: String,
  version: VersionNumber,
}
impl DevtoolsInspectable for Profile {}

/// The profiles whose latest version still names the archived resource, as the team server reads them now; none when
/// the stop does not say which resource it was, or the server cannot be asked.
fn still_using(ctx: &mut Ctx, task: &TaskContext, stop: &ArchiveStop) -> Vec<Profile> {
  let services = Services::of(ctx);
  let archived = stop.archived.as_ref().map(|archived: &Archived| archived.id);
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
          Ok::<_, ()>(
            used
              .resources
              .into_iter()
              .filter(|used| used.kind == SharedResourceKind::AgentProfile)
              .map(|used| Profile {
                id: used.id,
                name: used.name,
                version: used.version,
              })
              .collect(),
          )
        }
      },
    )
    .state()
    .get();
  state.data.unwrap_or_default()
}

/// A profile that still names the resource: its name and version, why it cannot run, and where to change it.
fn profile_row(ctx: &Ctx, (index, profile): (usize, &Profile), resource: &str, open: Open) -> Element {
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
  row(
    &id,
    Text::new(&name).variant(CardsText::Decided.role()),
    Some(Text::new(&need).variant(TypographyStyle::Description)),
    Some(link.into()),
  )
}

/// One row of the list: what it is about, what is the matter with it and, at the end, where to look.
fn row(id: &str, what: Text, need: Option<Text>, link: Option<Element>) -> Element {
  let mut row = Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(NOT_READY_ROW_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Loose.role())
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Snug.role()))
    .child(what.id(format!("{id}-what")));
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
