//! A row of All projects (`Project list row` `JSey9`) and the column headings above the rows (`Project list
//! header` `L5sps`): the project's initial on a tile, its name and folder — and why it cannot be opened —, where
//! it is (`Where` `mmabW` in 09 `i5G19A`), where it stands, its active runs and the runs that need the person, when it
//! was last opened, the × that asks whether to remove it from the list and, for a project that opens, the chevron.
//! Selecting such a row opens the project's Overview. While its removal is asked, the question sits under it on the
//! `hover` fill (`mykcc`).

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
  },
};

/// The columns (`W8VSrP`, `l0DHM`, `ILPuo`, `QbqqK`, `gG64B`, `b45tkg`), the tile (`jmNIU`), the chevron's slot
/// (`Sutrt`) and the headings' row (`L5sps`).
const WHERE_WIDTH: f32 = 160.0;
const STATUS_WIDTH: f32 = 168.0;
const COUNT_WIDTH: f32 = 88.0;
const OPENED_WIDTH: f32 = 96.0;
const ACTIONS_WIDTH: f32 = 56.0;
/// The room *Move to account* takes under the pointer: *Needs you* and *Last opened* with the 12 between them
/// (`LinZt`).
const MOVE_WIDTH: f32 = COUNT_WIDTH + 12.0 + OPENED_WIDTH;
const TILE: f32 = 28.0;
const OPEN_WIDTH: f32 = 20.0;
const HEADINGS_HEIGHT: f32 = 32.0;
/// Longest folder path shown in full in a row; a longer one keeps its drive and last folders, so the folder's
/// own name stays in view.
const PATH_CHARS: usize = 72;

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
    .child(Row::new().width(Dimension::Px(ACTIONS_WIDTH)))
}

/// What a row shows: the project, where it is (`None` while that is not read, or cannot be), whether an account is
/// signed in, what the screen asks, and whether it is the last, which draws no line under it.
#[derive(Clone)]
pub(super) struct RowProps {
  pub listed: ListedProject,
  pub place: Option<Place>,
  pub signed_in: bool,
  pub ask: Ask,
  pub last: bool,
}

impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.listed == other.listed
      && self.place == other.place
      && self.signed_in == other.signed_in
      && self.last == other.last
  }
}

impl DevtoolsInspectable for RowProps {}

/// The row of a project, with its removal or move question when it is asked. Under the pointer, with an account signed
/// in, it offers what moving it can do (`LinZt`, `xx4ST`): a component of its own, so the pointer moving over the list
/// renders only the rows it enters and leaves.
pub(super) struct ProjectRow {
  under_pointer: Signal<bool>,
}

impl Component for ProjectRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      under_pointer: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RowProps {
      listed,
      place,
      signed_in,
      ask,
      last,
    } = ctx.props::<RowProps>().clone();
    row(
      ctx,
      (&listed, place.as_ref(), signed_in),
      (&ask, &self.under_pointer),
      last,
    )
  }
}

fn row(
  ctx: &mut Ctx,
  (listed, place, signed_in): (&ListedProject, Option<&Place>, bool),
  (ask, under_pointer): (&Ask, &Signal<bool>),
  last: bool,
) -> Element {
  let id = listed.project.id;
  let asking = ask.removing.get().0 == Some(id);
  let moving = ask.moving.get().0.filter(|(project, _)| *project == id);
  let offer = move_project::offer(signed_in, listed, place);
  let hovered = under_pointer.get() && !asking && moving.is_none();
  let now = Services::of(ctx).clock.now();
  let opened = message_text(ctx, &display::elapsed(now, listed.project.last_opened_at));
  let cells = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(identity(ctx, listed))
    .child(where_cell(ctx, listed, place))
    .child(Row::new().width(Dimension::Px(STATUS_WIDTH)).child(status(ctx, listed)))
    .child(count(ctx, listed, Count::Active));
  let cells = match offer {
    Offer::Move(to) if hovered => cells.child(move_project::link(ctx, listed, to, ask, MOVE_WIDTH)),
    _ => cells.child(count(ctx, listed, Count::NeedsYou)).child(
      Text::new(&opened)
        .variant(TypographyStyle::Description)
        .nowrap()
        .width(Dimension::Px(OPENED_WIDTH)),
    ),
  }
  .child(actions(ctx, listed, ask, asking));
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
  let (enter, leave) = (under_pointer.clone(), under_pointer.clone());
  let mut block = Column::new()
    .id(format!("project-row-{id}"))
    .describe("project", listed.project.name.to_string())
    .describe("opens", if listed.opens() { "yes" } else { "no" })
    .width(Dimension::Pct(100.0))
    .on_mouse_enter(move || enter.set(true))
    .on_mouse_leave(move || leave.set(false))
    .child(cells)
    .with_children((hovered && offer == Offer::Shared).then(|| move_project::shared_note(ctx, id)));
  if asking {
    block = block
      .background(ExtraColor::Hover.role())
      .child(remove::question(ctx, listed, ask));
  }
  if let Some((_, to)) = moving {
    block = block.child(move_project::question(ctx, listed, to, ask));
  }
  if last {
    block.into()
  } else {
    block
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

/// The initial on its tile, the name, the folder and why the project cannot be opened (`Project cell` `IiUry`).
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
        .variant(ExtraTypography::MonoNote.role())
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
    .spacing(SpacingSize::Md)
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
      .variant(TypographyStyle::Description)
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

/// Where the project stands (`Status cell` `mI3Vr`): its folder or data first, then its runner.
fn status(ctx: &Ctx, listed: &ListedProject) -> Row {
  let (glyph, mark, words, key) = look(&listed.standing);
  Row::new()
    .id(format!("project-row-status-{}", listed.project.id))
    .describe("status", key)
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, mark))
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Label)
        .color(words)
        .nowrap(),
    )
}

/// The icon, its color, the words' color and the words' key of a standing (`f808M`, `TPFev`).
fn look(standing: &Standing) -> (Icon, PaletteColor, PaletteColor, &'static str) {
  match standing {
    Standing::FolderMissing => (
      Icon::FolderX,
      PaletteColor::Danger,
      PaletteColor::Danger,
      "projects.status.folderMissing",
    ),
    Standing::Unreadable(_) => (
      Icon::CircleAlert,
      PaletteColor::Danger,
      PaletteColor::Danger,
      "projects.status.unreadable",
    ),
    Standing::Opens(runner) => match runner {
      RunnerShown::NotStarted => (
        Icon::Circle,
        PaletteColor::TextMuted,
        PaletteColor::TextSecondary,
        "projects.status.notStarted",
      ),
      RunnerShown::Starting => (
        Icon::LoaderCircle,
        PaletteColor::TextSecondary,
        PaletteColor::TextSecondary,
        "projects.status.starting",
      ),
      RunnerShown::Online => (
        Icon::CircleDot,
        PaletteColor::Success,
        PaletteColor::TextSecondary,
        "projects.status.online",
      ),
      RunnerShown::Reconnecting => (
        Icon::RefreshCw,
        PaletteColor::TextSecondary,
        PaletteColor::TextSecondary,
        "projects.status.reconnecting",
      ),
      RunnerShown::Stopping => (
        Icon::LoaderCircle,
        PaletteColor::TextSecondary,
        PaletteColor::TextSecondary,
        "projects.status.stopping",
      ),
      RunnerShown::Stopped => (
        Icon::Unplug,
        PaletteColor::TextMuted,
        PaletteColor::TextSecondary,
        "projects.status.stopped",
      ),
      RunnerShown::Offline => (
        Icon::Unplug,
        PaletteColor::Danger,
        PaletteColor::Danger,
        "projects.status.offline",
      ),
    },
  }
}

#[derive(Clone, Copy)]
enum Count {
  Active,
  NeedsYou,
}

/// A count of the project's runs: a dash while it is not read and for a project that cannot be opened;
/// runs that need the person, when there are any, with the hand in `warning` (`Needs you cell` `S4mu9e`).
fn count(ctx: &Ctx, listed: &ListedProject, which: Count) -> Row {
  let name = match which {
    Count::Active => "active",
    Count::NeedsYou => "needs-you",
  };
  let cell = Row::new()
    .id(format!("project-row-{name}-{}", listed.project.id))
    .width(Dimension::Px(COUNT_WIDTH))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center);
  let value = listed.counts.map(|counts| match which {
    Count::Active => counts.active,
    Count::NeedsYou => counts.needs_you,
  });
  match (which, value) {
    (_, None) => cell.child(
      Text::new(&ctx.t("projects.unknownCount"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    ),
    (Count::NeedsYou, Some(waiting)) if waiting > 0 => {
      cell.child(icon(Icon::Hand, IconSize::Sm, PaletteColor::Warning)).child(
        Text::new(&waiting.to_string())
          .variant(TypographyStyle::Label)
          .color(PaletteColor::Warning),
      )
    }
    (_, Some(value)) => cell.child(Text::new(&value.to_string()).variant(TypographyStyle::Description)),
  }
}

/// The × and, for a project that opens, the chevron (`Actions` `t1Ze3`).
fn actions(ctx: &mut Ctx, listed: &ListedProject, ask: &Ask, asking: bool) -> Row {
  let (id, name) = (listed.project.id, listed.project.name.to_string());
  let removing = ask.removing.clone();
  let mut remove = IconButton::new(
    format!("project-remove-{id}"),
    Icon::X,
    ctx.t_args("projects.remove", [("name", name.as_str())]),
  )
  .small()
  .on_click(move || removing.update(|shown| shown.0 = if shown.0 == Some(id) { None } else { Some(id) }));
  if asking {
    remove = remove.active();
  }
  let open = Row::new()
    .width(Dimension::Px(OPEN_WIDTH))
    .justify(Justify::Center)
    .with_children(
      listed
        .opens()
        .then(|| icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)),
    );
  Row::new()
    .width(Dimension::Px(ACTIONS_WIDTH))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .justify(Justify::End)
    .child(remove.render(ctx))
    .child(open)
}
