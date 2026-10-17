//! A row of All projects (`Project list row` `JSey9`) and the column headings above the rows (`Project list
//! header` `L5sps`): the project's initial on a tile, its name and folder — and why it cannot be opened —, where
//! it stands, its active runs and the runs that need the person, when it was last opened, the × that asks
//! whether to remove it from the list and, for a project that opens, the chevron. Selecting such a row opens
//! the project's Overview. While its removal is asked, the question sits under it on the `hover` fill
//! (`mykcc`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display,
  project_list::{ListedProject, RunnerShown, Standing},
};

use super::{Ask, remove};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  services::Services,
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ProjectsText},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    message::message_text,
    pressable::opening,
  },
};

/// The columns (`W8VSrP`, `ILPuo`, `QbqqK`, `gG64B`, `b45tkg`), the tile (`jmNIU`), the chevron's slot (`Sutrt`)
/// and the headings' row (`L5sps`).
const STATUS_WIDTH: f32 = 168.0;
const COUNT_WIDTH: f32 = 88.0;
const OPENED_WIDTH: f32 = 96.0;
const ACTIONS_WIDTH: f32 = 56.0;
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
    .child(heading("projects.columns.status").width(Dimension::Px(STATUS_WIDTH)))
    .child(heading("projects.columns.activeRuns").width(Dimension::Px(COUNT_WIDTH)))
    .child(heading("projects.columns.needsYou").width(Dimension::Px(COUNT_WIDTH)))
    .child(heading("projects.columns.lastOpened").width(Dimension::Px(OPENED_WIDTH)))
    .child(Row::new().width(Dimension::Px(ACTIONS_WIDTH)))
}

/// The row of `listed`, with its removal question when it is asked; `last` draws no line under it.
pub(super) fn row(ctx: &mut Ctx, listed: &ListedProject, ask: &Ask, last: bool) -> Element {
  let id = listed.project.id;
  let asking = ask.removing.get().0 == Some(id);
  let now = Services::of(ctx).clock.now();
  let opened = message_text(ctx, &display::elapsed(now, listed.project.last_opened_at));
  let cells = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(identity(ctx, listed))
    .child(Row::new().width(Dimension::Px(STATUS_WIDTH)).child(status(ctx, listed)))
    .child(count(ctx, listed, Count::Active))
    .child(count(ctx, listed, Count::NeedsYou))
    .child(
      Text::new(&opened)
        .variant(TypographyStyle::Description)
        .nowrap()
        .width(Dimension::Px(OPENED_WIDTH)),
    )
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
  let mut block = Column::new()
    .id(format!("project-row-{id}"))
    .describe("project", listed.project.name.to_string())
    .describe("opens", if listed.opens() { "yes" } else { "no" })
    .width(Dimension::Pct(100.0))
    .child(cells);
  if asking {
    block = block
      .background(ExtraColor::Hover.role())
      .child(remove::question(ctx, listed, ask));
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
