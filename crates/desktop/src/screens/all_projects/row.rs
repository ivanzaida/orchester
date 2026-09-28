//! A row of All projects (`tBszO` and its siblings in `Z7IZZ`) and the column headings above the
//! rows (`b2EGz`): the project's initial, name and folder, where it stands, its active runs, the
//! runs that need the user, when it was last opened, the × that removes it from the list and, for
//! a project that can be opened, the chevron. Selecting such a row opens the project.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{
  run::{RunCounts, TaskPhase},
  time::Timestamp,
};

use super::OpenAction;
use crate::{
  display,
  messages::Message,
  runner::{Runners, state::RunnerState},
  tabs::Tabs,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, UNAVAILABLE_OPTION_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{opens_place, pressable, surface},
    status_label::status_label,
  },
  workspace::{ProjectData, ProjectEntry},
};

/// Column widths, the avatar and the remove control (`sMsn0`, `QtwKS`, `FwIZr`, `y2jM3b`,
/// `YRzUG`, `lmfC8`, `g00Id`, `uATmp`) and the heading row height (`b2EGz`).
const AVATAR_SIZE: f32 = 36.0;
const STATUS_WIDTH: f32 = 168.0;
const COUNT_WIDTH: f32 = 80.0;
const OPENED_WIDTH: f32 = 96.0;
const ACTIONS_WIDTH: f32 = 58.0;
const REMOVE_SIZE: f32 = 28.0;
const HEADINGS_HEIGHT: f32 = 32.0;

/// Where a row's project stands, from its entry and its data as far as they are read.
enum Standing<'a> {
  FolderMissing,
  Unreadable(&'a Message),
  /// It opens; the phases of its tasks once they are read.
  Openable(Option<&'a [TaskPhase]>),
}

impl<'a> Standing<'a> {
  fn of(entry: &ProjectEntry, data: Option<&'a ProjectData>) -> Self {
    match data {
      _ if entry.folder_missing => Self::FolderMissing,
      Some(ProjectData::Unreadable(reason)) => Self::Unreadable(reason),
      Some(ProjectData::Readable(phases)) => Self::Openable(phases.as_deref()),
      None => Self::Openable(None),
    }
  }
}

/// What every row reads besides its entry.
pub(super) struct Rows<'a> {
  pub(super) runners: &'a Runners,
  pub(super) open: &'a OpenAction,
  pub(super) tabs: &'a Tabs,
}

fn cell(text: &str, width: f32, color: PaletteColor) -> Text {
  Text::new(text)
    .variant(TypographyStyle::Caption)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .width(Dimension::Px(width))
}

pub(super) fn column_headings(ctx: &Ctx) -> Element {
  let heading = |key: &str, width: f32| cell(&ctx.t(key), width, PaletteColor::TextMuted);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADINGS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Row::new().width(Dimension::Px(AVATAR_SIZE)))
    .child(
      Text::new(&ctx.t("projects.columns.project"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .child(heading("projects.columns.status", STATUS_WIDTH))
    .child(heading("projects.columns.activeRuns", COUNT_WIDTH))
    .child(heading("projects.columns.needsYou", COUNT_WIDTH))
    .child(heading("projects.columns.lastOpened", OPENED_WIDTH))
    .child(Row::new().width(Dimension::Px(ACTIONS_WIDTH)))
    .into()
}

pub(super) fn project_row(
  ctx: &Ctx,
  index: usize,
  (entry, data): (&ProjectEntry, Option<&ProjectData>),
  rows: &Rows<'_>,
  remove: impl Fn() + Send + Sync + 'static,
) -> Element {
  let project = &entry.project;
  let runner = rows
    .runners
    .status_of(&project.repo_root)
    .map(|status| status.state_for(&project.repo_root));
  let online = runner == Some(RunnerState::Online);
  let standing = Standing::of(entry, data);
  // A dash until the counts are read: not known yet, never 0 (01.06 E `t8TEOZ`).
  let counts = match standing {
    Standing::Openable(Some(phases)) => Some(RunCounts::of(phases.iter().copied(), online)),
    Standing::Openable(None) | Standing::FolderMissing | Standing::Unreadable(_) => None,
  };
  let count = |value: fn(RunCounts) -> usize| {
    counts.map_or_else(
      || ctx.t("projects.unknownCount").to_string(),
      |counts| value(counts).to_string(),
    )
  };
  let opened = message_text(ctx, &display::elapsed(Timestamp::now(), project.last_opened_at));
  let openable = matches!(standing, Standing::Openable(_));
  let content = surface()
    .id(format!("project-row-{index}"))
    .describe("project", project.name.to_string())
    .flex(1.0)
    .padding_vertical(ExtraSpacing::Group.role())
    .padding_left(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(avatar(project.name.as_str(), openable))
    .child(identity(ctx, index, entry, &standing))
    .child(
      Row::new()
        .width(Dimension::Px(STATUS_WIDTH))
        .child(status(ctx, index, &standing, runner.as_ref())),
    )
    .child(cell(
      &count(|counts| counts.active),
      COUNT_WIDTH,
      PaletteColor::TextSecondary,
    ))
    .child(cell(
      &count(|counts| counts.needs_you),
      COUNT_WIDTH,
      PaletteColor::TextSecondary,
    ))
    .child(cell(&opened, OPENED_WIDTH, PaletteColor::TextSecondary));
  let content = if openable {
    let (action, repo_root) = (rows.open.clone(), project.repo_root.clone());
    opens_place(
      content.hovered_style(Style::new().background(PaletteColor::SurfaceRaised)),
      rows.tabs,
      move |open| action.run((repo_root.clone(), open)),
    )
  } else {
    content.focusable(false)
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Md)
    .padding_right(ExtraSpacing::Stack.role())
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(content)
    .child(actions(index, openable, remove))
    .into()
}

/// The project's initial on a tile (`g00Id`), faded while the project cannot be opened.
fn avatar(name: &str, openable: bool) -> Element {
  let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
  let tile = Row::new()
    .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(Text::new(&initial).variant(TypographyStyle::Label));
  if openable {
    tile.into()
  } else {
    tile.opacity(UNAVAILABLE_OPTION_OPACITY).into()
  }
}

/// Name, folder and, for a project that cannot be opened, why (`Hl5p2`, `ZIbjI`).
fn identity(ctx: &Ctx, index: usize, entry: &ProjectEntry, standing: &Standing<'_>) -> Element {
  let why = match standing {
    Standing::FolderMissing => Some(ctx.t("projects.folderMissingHint").to_string()),
    Standing::Unreadable(reason) => Some(message_text(ctx, reason).to_string()),
    Standing::Openable(_) => None,
  };
  Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Indicator.role())
    .child(Text::new(entry.project.name.as_str()).variant(TypographyStyle::Label))
    .child(
      Text::new(&entry.project.repo_root.display().to_string())
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(why.map(|why| {
      Text::new(&why)
        .id(format!("project-row-why-{index}"))
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0))
    }))
    .into()
}

/// Where the project stands (`yJuoV`): its folder or data first, then its runner in this session.
fn status(ctx: &Ctx, index: usize, standing: &Standing<'_>, runner: Option<&RunnerState>) -> Element {
  let (glyph, key) = match (standing, runner) {
    (Standing::FolderMissing, _) => (Icon::FolderX, "projects.status.folderMissing"),
    (Standing::Unreadable(_), _) => (Icon::AlertCircle, "projects.status.unreadable"),
    (Standing::Openable(_), None) => (Icon::Circle, "projects.status.notStarted"),
    (Standing::Openable(_), Some(state)) => runner_look(state),
  };
  Row::new()
    .id(format!("project-row-status-{index}"))
    .describe("status", key)
    .child(status_label(glyph, &ctx.t(key)))
    .into()
}

fn runner_look(state: &RunnerState) -> (Icon, &'static str) {
  match state {
    RunnerState::Starting => (Icon::Loader, "projects.status.starting"),
    RunnerState::Online => (Icon::CircleDot, "projects.status.online"),
    RunnerState::Reconnecting { .. } => (Icon::RefreshCw, "projects.status.reconnecting"),
    RunnerState::Stopping => (Icon::Loader, "projects.status.stopping"),
    RunnerState::Stopped => (Icon::Unplug, "projects.status.stopped"),
    RunnerState::Failed { .. } => (Icon::Unplug, "projects.status.offline"),
  }
}

/// The × that removes the project from the list, and the chevron of a project that opens (`a50sOk`).
fn actions(index: usize, openable: bool, remove: impl Fn() + Send + Sync + 'static) -> Element {
  let remove = pressable(surface().id(format!("project-remove-{index}")), remove)
    .size(Dimension::Px(REMOVE_SIZE), Dimension::Px(REMOVE_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary));
  let open: Element = if openable {
    icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary).into()
  } else {
    Row::new()
      .size(Dimension::Px(OPEN_WIDTH), Dimension::Px(OPEN_WIDTH))
      .into()
  };
  Row::new()
    .width(Dimension::Px(ACTIONS_WIDTH))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::End)
    .child(remove)
    .child(open)
    .into()
}

/// The chevron's width, kept empty on a row that does not open so the × lines up (`cVrx1`).
const OPEN_WIDTH: f32 = 14.0;
