//! A row of All projects (`Project · Kontur` `GP991` in 09 `i5G19A`) and the column headings above the rows (`Table
//! header` `C7osHr`): the project's initial on a tile, its name and folder — and why it cannot be opened —, where it is
//! (`Where` `mmabW`), where it stands as a badge (`Status` `D4YySp`), its active runs and the runs that need the
//! person, when it was last opened and, for a project that opens, the chevron. Each row is 56 high over a hairline.
//! Selecting a row that opens shows the project's Overview. The frame draws no way to remove a project at rest: the ×
//! that asks whether to remove it shows at the end of *Last opened* while the pointer is on the row and while its
//! removal is asked, as the tasks list shows its row actions under the pointer. While its removal is asked, the
//! question sits under the row on the `hover` fill (`mykcc`). With an account signed in, the pointer on a row also
//! offers to move it, in place of its last two columns with the × at the end ([`move_project`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display,
  project_list::{ListedProject, RunnerShown, Standing},
  sharing::Place,
};

use super::{
  Ask,
  move_project::{self, Offer},
  remove,
};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  services::Services,
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ProjectsText},
  ui::{
    chip::{BadgeTone, badge, cut_badge},
    icon::{Icon, icon},
    icon_button::IconButton,
    message::message_text,
    pressable::opening,
    skeleton::{self, bone},
  },
};

/// The columns (`l0DHM`, `z9COe`, `RNghn`, `SD8Wf`, `z1n28N`), the chevron's slot (`m7NhQ`), the tile (`pzQwU`), a
/// row (`GP991`) and the headings' row (`C7osHr`).
const WHERE_WIDTH: f32 = 160.0;
const STATUS_WIDTH: f32 = 168.0;
const COUNT_WIDTH: f32 = 80.0;
const OPENED_WIDTH: f32 = 96.0;
/// The room *Move to account* takes under the pointer: *Needs you* and *Last opened* with the 12 between them
/// (`LinZt`).
const MOVE_WIDTH: f32 = COUNT_WIDTH + 12.0 + OPENED_WIDTH;
const CHEVRON_WIDTH: f32 = 14.0;
const TILE: f32 = 28.0;
const ROW_HEIGHT: f32 = 56.0;
const HEADINGS_HEIGHT: f32 = 32.0;
/// Longest folder path shown in full in a row; a longer one keeps its drive and last folders, so the folder's
/// own name stays in view.
const PATH_CHARS: usize = 72;
/// A row's status, needs-you count and last-opened bones while the projects are read (`EZ5Bq`).
const BONE_STATUS: f32 = 120.0;
const BONE_COUNT: f32 = 26.0;
const BONE_OPENED: f32 = 62.0;

pub(super) fn headings(ctx: &Ctx) -> Row {
  let heading = |key: &str| {
    Text::new(&ctx.t(key))
      .variant(ProjectsText::ColumnHeading.role())
      .nowrap()
  };
  Row::new()
    .id("project-list-headings")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADINGS_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(heading("projects.columns.project").flex(1.0))
    .child(heading("projects.columns.where").width(Dimension::Px(WHERE_WIDTH)))
    .child(heading("projects.columns.status").width(Dimension::Px(STATUS_WIDTH)))
    .child(heading("projects.columns.activeRuns").width(Dimension::Px(COUNT_WIDTH)))
    .child(heading("projects.columns.needsYou").width(Dimension::Px(COUNT_WIDTH)))
    .child(heading("projects.columns.lastOpened").width(Dimension::Px(OPENED_WIDTH)))
    .child(Row::new().width(Dimension::Px(CHEVRON_WIDTH)))
}

/// A project's row while the projects are read (`12 · All projects · loading` `EZ5Bq`): its tile, name, folder, runner
/// status, counts, last opened and actions as bones in the row's own cells; `(name, folder)` are its words' lengths.
pub(super) fn bones(ctx: &Ctx, (name, folder): (f32, f32), last: bool) -> Row {
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(skeleton::line(ctx, ExtraTypography::Strong.role(), name))
    .child(skeleton::line(ctx, ExtraTypography::MonoNote.role(), folder));
  let identity = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(bone(TILE, TILE).rounded(RadiusSize::Md))
    .child(text);
  let cell = |width: f32| Row::new().width(Dimension::Px(width)).align_items(Alignment::Center);
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(identity)
    .child(cell(WHERE_WIDTH).child(skeleton::line(ctx, TypographyStyle::Description, BONE_STATUS)))
    .child(cell(STATUS_WIDTH).child(skeleton::status(ctx, BONE_STATUS)))
    .child(cell(COUNT_WIDTH).child(skeleton::count()))
    .child(cell(COUNT_WIDTH).child(bone(BONE_COUNT, 8.0)))
    .child(cell(OPENED_WIDTH).child(skeleton::line(ctx, TypographyStyle::Description, BONE_OPENED)))
    .child(cell(CHEVRON_WIDTH).child(bone(CHEVRON_WIDTH, CHEVRON_WIDTH)));
  if last {
    row
  } else {
    row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  }
}

/// The row of `listed` and where it is (`None` while that is not read, or cannot be), whether an account is signed in,
/// with its removal or move question when it is asked.
pub(super) fn row(
  ctx: &mut Ctx,
  (listed, place, signed_in): (&ListedProject, Option<&Place>, bool),
  ask: &Ask,
) -> Element {
  let now = Services::of(ctx).clock.now();
  let props = RowProps {
    listed: listed.clone(),
    place: place.cloned(),
    signed_in,
    opened: message_text(ctx, &display::elapsed(now, listed.project.last_opened_at)),
    ask: ask.clone(),
  };
  ctx.mount_keyed::<ProjectRow>(&format!("project-row-{}", listed.project.id), props)
}

#[derive(Clone)]
struct RowProps {
  listed: ListedProject,
  place: Option<Place>,
  signed_in: bool,
  /// When it was last opened, in words.
  opened: Arc<str>,
  ask: Ask,
}

impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.listed, &self.place, self.signed_in, &self.opened)
      == (&other.listed, &other.place, other.signed_in, &other.opened)
  }
}

impl DevtoolsInspectable for RowProps {}

/// A component of its own, so the pointer moving over the list renders only the rows it enters and leaves. Under the
/// pointer, with an account signed in, it offers what moving it can do (`LinZt`, `xx4ST`).
struct ProjectRow {
  hovered: Signal<bool>,
}

impl Component for ProjectRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let (listed, ask) = (&props.listed, &props.ask);
    let id = listed.project.id;
    let asking = ask.removing.get().0 == Some(id);
    let moving = ask.moving.get().0.filter(|(project, _)| *project == id);
    let offer = move_project::offer(props.signed_in, listed, props.place.as_ref());
    let offered = self.hovered.get() && !asking && moving.is_none();
    let cells = cells(ctx, &props, (self.hovered.get(), asking), offered.then_some(offer));
    let cells: Element = if listed.opens() && !asking {
      let navigation = Navigation::of(ctx);
      opening(navigation.to(Route::Project(id, ProjectPage::Overview(OverviewTab::default()))))
        .width(Dimension::Pct(100.0))
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .child(cells)
        .into()
    } else {
      cells.into()
    };
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let mut block = Column::new()
      .id(format!("project-row-{id}"))
      .describe("project", listed.project.name.to_string())
      .describe("opens", if listed.opens() { "yes" } else { "no" })
      .width(Dimension::Pct(100.0))
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(cells)
      .with_children((offered && offer == Offer::Shared).then(|| move_project::shared_note(ctx, id)));
    if asking {
      block = block
        .background(ExtraColor::Hover.role())
        .child(remove::question(ctx, listed, ask));
    }
    if let Some((_, to)) = moving {
      block = block.child(move_project::question(ctx, listed, to, ask));
    }
    block
  }
}

/// The row's cells. Under the pointer the × shows at the end of *Last opened*, or, where the row offers a move, at the
/// end of the move in place of the last two columns; `offer` is what the row offers while it is offered.
fn cells(ctx: &mut Ctx, props: &RowProps, (hovered, asking): (bool, bool), offer: Option<Offer>) -> Row {
  let RowProps {
    listed,
    place,
    opened,
    ask,
    ..
  } = props;
  let remove = (hovered || asking).then(|| remove_button(ctx, listed, ask, asking));
  let cells = Row::new()
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(identity(ctx, listed))
    .child(where_cell(ctx, listed, place.as_ref()))
    .child(status(ctx, listed))
    .child(count(ctx, listed, Count::Active));
  let cells = match offer {
    Some(Offer::Move(to)) => cells.child(
      Row::new()
        .width(Dimension::Px(MOVE_WIDTH))
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .justify(Justify::End)
        .child(move_project::link(ctx, listed, to, ask))
        .with_children(remove),
    ),
    _ => cells
      .child(count(ctx, listed, Count::NeedsYou))
      .child(last_opened(opened, remove)),
  };
  cells.child(chevron(listed))
}

/// The initial on its tile, 10 before the name over the folder, and why the project cannot be opened (`Project`
/// `M6kSXp`).
fn identity(ctx: &Ctx, listed: &ListedProject) -> Row {
  let name = listed.project.name.as_str();
  let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
  let tile = Row::new()
    .width(Dimension::Px(TILE))
    .height(Dimension::Px(TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&initial).variant(ProjectsText::Initial.role()));
  let tile = if listed.opens() {
    tile
  } else {
    tile.opacity(DISABLED_OPACITY)
  };
  let why = match &listed.standing {
    Standing::FolderMissing => Some(ctx.t("projects.folderMissingHint")),
    Standing::Unreadable(reason) => Some(message_text(ctx, reason)),
    Standing::Opens(_) => None,
  };
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(Text::new(name).variant(ExtraTypography::Strong.role()).nowrap())
    .child(
      Text::new(&display::shorten_path(&listed.project.repo_root, PATH_CHARS))
        .variant(ProjectsText::Folder.role())
        .nowrap()
        .max_width(Dimension::Pct(100.0))
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(why.map(|why| {
      Text::new(&why)
        .id(format!("project-row-why-{}", listed.project.id))
        .variant(ProjectsText::Hint.role())
        .width(Dimension::Pct(100.0))
    }));
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(tile)
    .child(text)
}

/// Where the project is (`Where` in `LinZt`): a badge saying *Shared · Acme* in `info`, or *This computer* or *Your
/// account* in `text_secondary`; a dash while it is not read, and when its folder or data cannot be read. The team's
/// name is the server's own, of any length: a long one is cut to the column, whole in a tooltip.
fn where_cell(ctx: &mut Ctx, listed: &ListedProject, place: Option<&Place>) -> Row {
  let id = format!("project-row-where-{}", listed.project.id);
  let badge_id = format!("{id}-badge");
  let shown: Element = match place {
    None => Text::new(&ctx.t("projects.where.unknown"))
      .variant(ProjectsText::Value.role())
      .color(PaletteColor::TextMuted)
      .into(),
    Some(Place::Local) => {
      let key = if listed.project.owner.is_some() {
        "projects.where.account"
      } else {
        "projects.where.thisComputer"
      };
      badge(&badge_id, BadgeTone::Neutral, &ctx.t(key)).into()
    }
    Some(Place::Shared { team: Some(team) }) => {
      let words = ctx.t_args("projects.where.shared", [("team", team.as_str())]);
      cut_badge(ctx, &badge_id, BadgeTone::Info, &words).into()
    }
    Some(Place::Shared { team: None }) => {
      badge(&badge_id, BadgeTone::Info, &ctx.t("projects.where.sharedUnknown")).into()
    }
  };
  Row::new()
    .id(id)
    .width(Dimension::Px(WHERE_WIDTH))
    .align_items(Alignment::Center)
    .child(shown)
}

/// Where the project stands, its folder or data first, then its runner, as a badge (`Badge · Runner online` `W6Eiu`,
/// `Badge · Runner stopped` `t4d5DY`).
fn status(ctx: &Ctx, listed: &ListedProject) -> Row {
  let (tone, key) = look(&listed.standing);
  let id = format!("project-row-status-{}", listed.project.id);
  Row::new()
    .id(id.clone())
    .describe("status", key)
    .width(Dimension::Px(STATUS_WIDTH))
    .align_items(Alignment::Center)
    .child(badge(&format!("{id}-badge"), tone, &ctx.t(key)))
}

/// The tone and the words' key of a standing: an online runner in `success`, what keeps the project from working in
/// `danger`, the rest without a tone.
const fn look(standing: &Standing) -> (BadgeTone, &'static str) {
  match standing {
    Standing::FolderMissing => (BadgeTone::Danger, "projects.status.folderMissing"),
    Standing::Unreadable(_) => (BadgeTone::Danger, "projects.status.unreadable"),
    Standing::Opens(runner) => match runner {
      RunnerShown::NotStarted => (BadgeTone::Neutral, "projects.status.notStarted"),
      RunnerShown::Starting => (BadgeTone::Neutral, "projects.status.starting"),
      RunnerShown::Online => (BadgeTone::Success, "projects.status.online"),
      RunnerShown::Reconnecting => (BadgeTone::Neutral, "projects.status.reconnecting"),
      RunnerShown::Stopping => (BadgeTone::Neutral, "projects.status.stopping"),
      RunnerShown::Stopped => (BadgeTone::Neutral, "projects.status.stopped"),
      RunnerShown::Offline => (BadgeTone::Danger, "projects.status.offline"),
    },
  }
}

#[derive(Clone, Copy)]
enum Count {
  Active,
  NeedsYou,
}

/// A count of the project's runs in 12 (`gTVG7`, `fmJXP`): a dash while it is not read and for a project that cannot
/// be opened, none in `text_muted`, runs that need the person in `warning`.
fn count(ctx: &Ctx, listed: &ListedProject, which: Count) -> Row {
  let name = match which {
    Count::Active => "active",
    Count::NeedsYou => "needs-you",
  };
  let value = listed.counts.map(|counts| match which {
    Count::Active => counts.active,
    Count::NeedsYou => counts.needs_you,
  });
  let (words, color) = match (which, value) {
    (_, None) => (ctx.t("projects.unknownCount"), PaletteColor::TextMuted),
    (_, Some(0)) => (Arc::from("0"), PaletteColor::TextMuted),
    (Count::NeedsYou, Some(waiting)) => (Arc::from(waiting.to_string()), PaletteColor::Warning),
    (Count::Active, Some(active)) => (Arc::from(active.to_string()), PaletteColor::TextSecondary),
  };
  Row::new()
    .id(format!("project-row-{name}-{}", listed.project.id))
    .width(Dimension::Px(COUNT_WIDTH))
    .align_items(Alignment::Center)
    .child(Text::new(&words).variant(ProjectsText::Value.role()).color(color))
}

/// When the project was last opened (`RO2ay`), and the × at its end while it shows.
fn last_opened(opened: &str, remove: Option<Element>) -> Row {
  Row::new()
    .width(Dimension::Px(OPENED_WIDTH))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Text::new(opened)
        .variant(ProjectsText::Value.role())
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(remove)
}

/// The × that asks whether to remove the project from the list, pressed while that is asked.
fn remove_button(ctx: &mut Ctx, listed: &ListedProject, ask: &Ask, asking: bool) -> Element {
  let (id, name) = (listed.project.id, listed.project.name.to_string());
  let removing = ask.removing.clone();
  let remove = IconButton::new(
    format!("project-remove-{id}"),
    Icon::X,
    ctx.t_args("projects.remove", [("name", name.as_str())]),
  )
  .small()
  .on_click(move || removing.update(|shown| shown.0 = if shown.0 == Some(id) { None } else { Some(id) }));
  if asking { remove.active() } else { remove }.render(ctx)
}

/// The chevron of a project that opens (`Chevron` `P4PRH`).
fn chevron(listed: &ListedProject) -> Row {
  Row::new()
    .width(Dimension::Px(CHEVRON_WIDTH))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .with_children(
      listed
        .opens()
        .then(|| icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)),
    )
}
