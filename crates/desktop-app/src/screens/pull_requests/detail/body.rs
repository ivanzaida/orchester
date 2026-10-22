//! Under a pull request's header on Files changed: the toolbar ([`super::toolbar`]); why the latest read failed, if it
//! did; then the changed files (`File list` `sLQan`: FILES and their count, Find a file, the task page's file rows,
//! and the feedback threads, `Threads head` `kVI2F`) beside the selected file's diff with the task page's diff view,
//! feedback drawn into it; or what stands in their place (`c07NAC`): the pull request being read, too large to show,
//! without changes, a diff Orchester cannot read.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{diff::Diff, pull_request::PullRequest};
use orchester_workspace::{
  messages::Message,
  pull_requests::{
    detail::ReadDiff,
    page::{Body, PullRequestView},
  },
  tasks::changes::ReadChanges,
};

use super::{
  super::{labels::grouped, notice::detail_notice},
  feedback::{self, Env},
  toolbar::{Fixed, Instead, InsteadWhy, Range, head_notice, toolbar},
};
use crate::{
  routes::ChangeSource,
  screens::task::changes::{
    arrow_keys,
    diff::{CutAction, CutNote, DiffView, DiffViewProps},
    files::{FILE_LIST_WIDTH, FileItem, FileList, FileListProps, FileSummary, SelectFile},
    listed::FileListing,
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing, PullRequestsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::EmptyState,
  },
};

/// The widest a state's card is and the side of the reading glyph's tile (`aqHH2`).
const CARD_WIDTH: f32 = 600.0;
const READING_TILE: f32 = 36.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the body is shown with.
pub(super) struct Shown {
  /// The path of the file whose diff shows, as the route names it; the first while it names none.
  pub selected: Option<Arc<str>>,
  /// Opens another file where the press asks; in the active tab as a new entry of its history.
  pub select: SelectFile,
  pub find: Signal<String>,
  /// Open in GitHub, once the pull request's address is known.
  pub open: Option<Handler>,
  pub reading: bool,
  /// The diff shown instead of the pull request's latest.
  pub instead: Option<Instead>,
  /// Fixes the range can show what changed since their request.
  pub fixed: Vec<Fixed>,
  /// The pull request's feedback, once read: drawn into the diff and listed beside the files.
  pub feedback: Option<Env>,
  pub state: feedback::FeedbackState,
  /// Why the latest read of the feedback failed; `feedback` is then the one read before.
  pub feedback_failed: Option<Message>,
  /// Why the diff asked for instead of the latest could not be read; the latest shows.
  pub instead_failed: Option<Message>,
}

pub(super) fn body(ctx: &mut Ctx, view: &PullRequestView, shown: &Shown) -> Element {
  let column = Column::new()
    .id("pull-request-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  let column = match &view.pull_request {
    Some(pull_request) => column.child(toolbar(
      ctx,
      &Range {
        view,
        pull_request,
        instead: shown.instead.as_ref(),
        fixed: &shown.fixed,
        state: shown.feedback.as_ref().map(|_| &shown.state),
      },
    )),
    None => column,
  };
  let column = column.with_children(notices(ctx, view, shown));
  let content = match (&shown.instead, view.body()) {
    (Some(instead), _) => files(ctx, (&instead.read, view), shown),
    (None, Body::Files(read)) => files(ctx, (&read, view), shown),
    (None, Body::NotRead) if shown.reading => reading(ctx),
    (None, Body::NotRead) => card(
      ctx,
      (
        Icon::GitPullRequest,
        "pull_requests.detail.notRead.title",
        &ctx.t("pull_requests.detail.notRead.detail"),
      ),
      None,
    ),
    (None, Body::NoChanges) => card(
      ctx,
      (
        Icon::GitCompare,
        "pull_requests.detail.noChanges.title",
        &ctx.t("pull_requests.detail.noChanges.detail"),
      ),
      None,
    ),
    (None, Body::TooLarge) => {
      let detail = too_large(ctx, view.pull_request.as_ref());
      card(
        ctx,
        (Icon::Copy, "pull_requests.detail.tooLarge.title", &detail),
        shown.open.clone(),
      )
    }
    (None, Body::Unreadable) => card(
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

/// What failed, under the toolbar: the latest read of the pull request, the diff asked for instead of the latest,
/// and the latest read of its feedback, whose threads shown are then the ones read before.
fn notices(ctx: &Ctx, view: &PullRequestView, shown: &Shown) -> Vec<Element> {
  let read = view
    .latest_failure()
    .map(|failure| detail_notice(ctx, failure, view.synced_at));
  let instead = shown.instead_failed.as_ref().map(|error| -> Element {
    Notice::new(
      "pull-request-instead-failed",
      NoticeTone::Danger,
      ctx.t("pull_requests.feedback.range.readFailed"),
    )
    .body(message_text(ctx, error))
    .into()
  });
  let padded = read.into_iter().chain(instead).map(|notice| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Md)
          .left(PullRequestsSpacing::Detail.role())
          .right(PullRequestsSpacing::Detail.role()),
      )
      .child(notice)
      .into()
  });
  let feedback = shown
    .feedback_failed
    .as_ref()
    .map(|error| feedback::read_failed(ctx, error));
  padded.chain(feedback).collect()
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

/// The files of `all` whose path holds `find`, in any case.
fn found(all: &FileListing, find: &str) -> FileListing {
  let find = find.trim().to_lowercase();
  FileListing {
    keys: all.keys.clone(),
    files: Arc::new(
      all
        .files
        .iter()
        .filter(|listed| find.is_empty() || listed.path.to_lowercase().contains(&find))
        .cloned()
        .collect(),
    ),
  }
}

/// The files Find a file leaves, the selected one's diff beside them, feedback drawn into it.
fn files(ctx: &mut Ctx, (read, view): (&Arc<ReadDiff>, &PullRequestView), shown: &Shown) -> Element {
  let changes = ReadChanges {
    from: read.base.clone(),
    diff: read.diff.clone(),
    key: Arc::from(read.head.as_str()),
  };
  let all = FileListing::of(Some(&changes), None);
  let listing = found(&all, &shown.find.get());
  let position = shown
    .selected
    .as_ref()
    .and_then(|path| listing.files.iter().position(|listed| listed.path == *path))
    .unwrap_or_default();
  let select = shown.select.clone();
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
  let since = shown
    .instead
    .as_ref()
    .filter(|instead| instead.why == InsteadWhy::Since);
  let total = view.file_count().unwrap_or(all.files.len());
  let list = Column::new()
    .id("pull-request-files")
    .width(Dimension::Px(FILE_LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(files_head(ctx, (all.files.len(), since.map(|_| total)), &shown.find))
    .child(ctx.mount_keyed::<FileList>(
      "pull-request-file-list",
      FileListProps {
        items: Arc::new(items),
        select: select.clone(),
      },
    ))
    .with_children(shown.feedback.as_ref().map(|env| feedback::threads(ctx, env)));
  let latest = match &view.diff {
    orchester_workspace::pull_requests::page::PageDiff::Read(read) => Some(read.diff.clone()),
    _ => None,
  };
  let diff = listing
    .files
    .get(position)
    .map(|listed| diff_view(ctx, (read, &changes, listed), (shown, latest)));
  let notice = match (&shown.instead, &view.pull_request, &shown.feedback) {
    (Some(instead), Some(pull_request), Some(_)) => head_notice(ctx, (instead, pull_request.head_sha(), &shown.state)),
    _ => None,
  };
  Row::new()
    .id("pull-request-files-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .on_key_down(arrow_keys(&listing, position, select))
    .child(list)
    .child(
      Column::new()
        .flex(1.0)
        .height(Dimension::Pct(100.0))
        .min_width(Dimension::Px(0.0))
        .with_children(notice)
        .with_children(diff),
    )
    .into()
}

/// The selected file's diff, feedback drawn into it once read.
fn diff_view(
  ctx: &mut Ctx,
  (read, changes, listed): (
    &Arc<ReadDiff>,
    &ReadChanges,
    &crate::screens::task::changes::listed::Listed,
  ),
  (shown, latest): (&Shown, Option<Arc<Diff>>),
) -> Element {
  let file = &read.diff.files()[listed.index];
  let annotations = match (&shown.feedback, &shown.instead) {
    (Some(env), None)
    | (
      Some(env),
      Some(Instead {
        why: InsteadWhy::Earlier { .. },
        ..
      }),
    ) => Some(feedback::annotations(env, (&read.head, file, &listed.path), latest)),
    _ => None,
  };
  ctx.mount_keyed::<DiffView>(
    "pull-request-diff-view",
    DiffViewProps {
      diff: read.diff.clone(),
      key: changes.key.clone(),
      index: listed.index,
      file: format!("{}:{}", ChangeSource::Checkpoint.name(), listed.path).into(),
      source: Arc::from(""),
      open: None,
      viewed: Some(ctx.t("pull_requests.detail.viewedPlanned")),
      cut: cut_note(ctx, shown.open.clone()),
      annotations,
    },
  )
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

/// FILES and their count over Find a file (`Files head` `WOZwy`), 10 inside the list's own 8 at the sides; with what
/// a request's range leaves out of the pull request's `total` files (`of 6 · since your request`, `M1u6GA`).
fn files_head(ctx: &mut Ctx, (count, of): (usize, Option<usize>), find: &Signal<String>) -> Column {
  let field = FieldInput::new(
    "pull-request-find-file",
    ctx.t("pull_requests.detail.files.find"),
    find.clone(),
  )
  .search()
  .clearable(ctx.t("pull_requests.detail.files.clear"))
  .placeholder(ctx.t("pull_requests.detail.files.findPlaceholder"))
  .render(ctx);
  let of = of.map(|total| {
    Text::new(&ctx.t_args("pull_requests.feedback.range.files", [("total", total.to_string())]))
      .id("pull-request-files-of")
      .variant(TypographyStyle::Caption)
      .nowrap()
  });
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
        .child(Text::new(&count.to_string()).variant(PullRequestsText::FileCount.role()))
        .with_children(of),
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
