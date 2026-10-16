//! Under a pull request's header: the toolbar (`Changes toolbar` `C4AKw`) with the revision range, locked
//! since choosing another is planned, and the totals; why the latest read failed, if it did; then the changed
//! files (`File list` `sLQan`: FILES and their count, Find a file, the task page's file rows) beside the
//! selected file's diff with the task page's diff view, or what stands in their place (`c07NAC`): the pull
//! request being read, too large to show, without changes, a diff Orchester cannot read.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::pull_request::PullRequest;
use orchester_workspace::{
  display::short_revision,
  messages::Message,
  pull_requests::{
    detail::ReadDiff,
    page::{Body, PullRequestView},
  },
  tasks::changes::ReadChanges,
};

use super::super::{labels::grouped, notice::detail_notice};
use crate::{
  routes::ChangeSource,
  screens::task::changes::{
    arrow_keys,
    diff::{CutAction, CutNote, DiffView, DiffViewProps},
    files::{FILE_LIST_WIDTH, FileItem, FileList, FileListProps, FileSummary},
    listed::{FileListing, Listed},
  },
  theme::{
    ControlHeight, DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing,
    PullRequestsText,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::EmptyState,
    unavailable::reason_line,
  },
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// The toolbar's height (`C4AKw`), the range's width (`jHagN`), the widest a state's card is and the side
/// of the reading glyph's tile (`aqHH2`).
const TOOLBAR_HEIGHT: f32 = 44.0;
const RANGE_WIDTH: f32 = 248.0;
const CARD_WIDTH: f32 = 600.0;
const READING_TILE: f32 = 36.0;

/// What the body is shown with.
pub(super) struct Shown {
  pub selected: Signal<Option<Arc<str>>>,
  pub find: Signal<String>,
  /// Open in GitHub, once the pull request's address is known.
  pub open: Option<Handler>,
  pub reading: bool,
}

pub(super) fn body(ctx: &mut Ctx, view: &PullRequestView, shown: &Shown) -> Element {
  let column = Column::new()
    .id("pull-request-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  let column = match &view.pull_request {
    Some(pull_request) => column.child(toolbar(ctx, view, pull_request)),
    None => column,
  };
  let notice = view
    .latest_failure()
    .map(|failure| detail_notice(ctx, failure, view.synced_at));
  let column = column.with_children(notice.map(|notice| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Md)
          .left(PullRequestsSpacing::Detail.role())
          .right(PullRequestsSpacing::Detail.role()),
      )
      .child(notice)
  }));
  let content = match view.body() {
    Body::Files(read) => files(ctx, &read, shown),
    Body::NotRead if shown.reading => reading(ctx),
    Body::NotRead => card(
      ctx,
      (
        Icon::GitPullRequest,
        "pull_requests.detail.notRead.title",
        &ctx.t("pull_requests.detail.notRead.detail"),
      ),
      None,
    ),
    Body::NoChanges => card(
      ctx,
      (
        Icon::GitCompare,
        "pull_requests.detail.noChanges.title",
        &ctx.t("pull_requests.detail.noChanges.detail"),
      ),
      None,
    ),
    Body::TooLarge => {
      let detail = too_large(ctx, view.pull_request.as_ref());
      card(
        ctx,
        (Icon::Copy, "pull_requests.detail.tooLarge.title", &detail),
        shown.open.clone(),
      )
    }
    Body::Unreadable => card(
      ctx,
      (
        Icon::FileX,
        "pull_requests.detail.unreadable.title",
        &ctx.t("pull_requests.detail.unreadable.detail"),
      ),
      shown.open.clone(),
    ),
  };
  column.child(content).into()
}

/// Reading the stored pull request failed, or the read could not run at all.
pub(super) fn load_failed(ctx: &Ctx, error: &Message) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .left(PullRequestsSpacing::Detail.role())
        .right(PullRequestsSpacing::Detail.role()),
    )
    .child(
      Notice::new(
        "pull-request-load-failed",
        NoticeTone::Danger,
        ctx.t("pull_requests.detail.loadFailed"),
      )
      .body(message_text(ctx, error)),
    )
    .into()
}

/// The range, locked, and why; the totals at the end.
fn toolbar(ctx: &Ctx, view: &PullRequestView, pull_request: &PullRequest) -> Row {
  let head = short_revision(pull_request.head_sha().as_str());
  let range = match &view.diff {
    orchester_workspace::pull_requests::page::PageDiff::Read(read) => ctx.t_args(
      "pull_requests.detail.range",
      [("base", short_revision(read.base.as_str())), ("head", head)],
    ),
    _ => ctx.t_args("pull_requests.detail.rangeHead", [("head", head)]),
  };
  let range = Row::new()
    .id("pull-request-range")
    .width(Dimension::Px(RANGE_WIDTH))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .opacity(DISABLED_OPACITY)
    .describe("state", "unavailable")
    .child(
      Text::new(&range)
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .nowrap(),
    )
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted));
  let changes = pull_request.changes();
  let key = if changes.files == 1 {
    "pull_requests.detail.total.one"
  } else {
    "pull_requests.detail.total.other"
  };
  let total = ctx.t_args(
    key,
    [
      ("files", grouped(ctx, changes.files)),
      ("additions", grouped(ctx, changes.additions)),
      ("deletions", grouped(ctx, changes.deletions)),
    ],
  );
  Row::new()
    .id("pull-request-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding(Padding::horizontal(PullRequestsSpacing::Detail.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(range)
    .child(Row::new().child(
      reason_line("pull-request-range-reason", &ctx.t("pull_requests.detail.rangePlanned")).width(Dimension::Auto),
    ))
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&total)
        .id("pull-request-total")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
}

/// The files Find a file leaves, the selected one's diff beside them.
fn files(ctx: &mut Ctx, read: &Arc<ReadDiff>, shown: &Shown) -> Element {
  let changes = ReadChanges {
    from: read.base.clone(),
    diff: read.diff.clone(),
    key: Arc::from(read.head.as_str()),
  };
  let all = FileListing::of(Some(&changes), None);
  let find = shown.find.get().trim().to_lowercase();
  let listing = FileListing {
    keys: all.keys.clone(),
    files: Arc::new(
      all
        .files
        .iter()
        .filter(|listed| find.is_empty() || listed.path.to_lowercase().contains(&find))
        .cloned()
        .collect(),
    ),
  };
  let selected = shown.selected.get();
  let position = selected
    .as_ref()
    .and_then(|path| listing.files.iter().position(|listed| listed.path == *path))
    .unwrap_or_default();
  let chosen = shown.selected.clone();
  let select: Arc<dyn Fn(&Listed) + Send + Sync> =
    Arc::new(move |listed: &Listed| chosen.set(Some(listed.path.clone())));
  let diff_files = read.diff.files();
  let items: Vec<FileItem> = listing
    .files
    .iter()
    .enumerate()
    .map(|(at, listed)| FileItem::File {
      position: at,
      listed: listed.clone(),
      summary: FileSummary::of(&diff_files[listed.index]),
      selected: at == position,
    })
    .collect();
  let list = Column::new()
    .id("pull-request-files")
    .width(Dimension::Px(FILE_LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(files_head(ctx, all.files.len(), &shown.find))
    .child(ctx.mount_keyed::<FileList>(
      "pull-request-file-list",
      FileListProps {
        items: Arc::new(items),
        select: select.clone(),
      },
    ));
  let diff = listing.files.get(position).map(|listed| {
    ctx.mount_keyed::<DiffView>(
      "pull-request-diff-view",
      DiffViewProps {
        diff: read.diff.clone(),
        key: changes.key.clone(),
        index: listed.index,
        file: format!("{}:{}", ChangeSource::Checkpoint.name(), listed.path).into(),
        source: Arc::from(""),
        viewed: Some(ctx.t("pull_requests.detail.viewedPlanned")),
        cut: cut_note(ctx, shown.open.clone()),
      },
    )
  });
  Row::new()
    .id("pull-request-files-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .on_key_down(arrow_keys(&listing, position, select))
    .child(list)
    .with_children(diff)
    .into()
}

/// The note under a file cut short: the rest is read in GitHub, with Open in GitHub once the address is
/// known (`Long file cut` `F5MYhZ`).
fn cut_note(ctx: &Ctx, open: Option<Handler>) -> CutNote {
  CutNote {
    key: "pull_requests.detail.omitted",
    action: open.map(|run| CutAction {
      id: Arc::from("pull-request-cut-github"),
      icon: Icon::ExternalLink,
      label: ctx.t("pull_requests.row.openInGitHub"),
      run,
    }),
  }
}

/// FILES and their count over Find a file (`Files head` `WOZwy`), 10 inside the list's own 8 at the sides.
fn files_head(ctx: &mut Ctx, count: usize, find: &Signal<String>) -> Column {
  let field = FieldInput::new(
    "pull-request-find-file",
    ctx.t("pull_requests.detail.files.find"),
    find.clone(),
  )
  .search()
  .clearable(ctx.t("pull_requests.detail.files.clear"))
  .placeholder(ctx.t("pull_requests.detail.files.findPlaceholder"))
  .render(ctx);
  let head = Column::new()
    .id("pull-request-files-head")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .id("pull-request-files-heading")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("pull_requests.detail.files.heading")).variant(ExtraTypography::ColumnHeading.role()))
        .child(Text::new(&count.to_string()).variant(PullRequestsText::FileCount.role())),
    )
    .child(field);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .bottom(SpacingSize::Sm)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .child(head)
}

/// The pull request is read: the turning loader on its tile, what is read and how, centered in the body
/// (`Reading` `aqHH2`).
fn reading(ctx: &Ctx) -> Element {
  let tile = Row::new()
    .id("pull-request-reading-glyph")
    .width(Dimension::Px(READING_TILE))
    .height(Dimension::Px(READING_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::LoaderCircle, IconSize::Xl, PaletteColor::TextSecondary));
  Column::new()
    .id("pull-request-reading")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .describe("state", "loading")
    .child(tile)
    .child(
      Column::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("pull_requests.detail.reading.title"))
            .id("pull-request-reading-title")
            .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(&ctx.t("pull_requests.detail.reading.detail"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .into()
}

/// A state's card: its glyph, title and why, with Open in GitHub when the pull request is better read there.
fn card(ctx: &Ctx, (glyph, title, detail): (Icon, &str, &str), open: Option<Handler>) -> Element {
  let mut state = EmptyState::new("pull-request-state", glyph, ctx.t(title), detail.to_owned());
  if let Some(open) = open {
    state = state.action(
      ActionButton::new(
        "pull-request-state-github",
        ButtonKind::Secondary,
        ctx.t("pull_requests.row.openInGitHub"),
      )
      .icon(Icon::ExternalLink)
      .on_click(move || open()),
    );
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Roomy.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .max_width(Dimension::Px(CARD_WIDTH))
        .child(state),
    )
    .into()
}

/// How much the pull request changes, against what Orchester shows.
fn too_large(ctx: &Ctx, pull_request: Option<&PullRequest>) -> Arc<str> {
  let Some(pull_request) = pull_request else {
    return ctx.t("pull_requests.detail.tooLarge.unknown");
  };
  let changes = pull_request.changes();
  ctx.t_args(
    "pull_requests.detail.tooLarge.detail",
    [
      ("files", grouped(ctx, changes.files)),
      ("additions", grouped(ctx, changes.additions)),
      ("deletions", grouped(ctx, changes.deletions)),
    ],
  )
}
