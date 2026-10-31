//! The parts of a shared project and how each stands (`Progress list` `hiH6i`), in one outlined box with a line between
//! two: *Joined Acme* (`Ni2Ui`), *Linked to …* (`aiQTB`), the tasks (`ZtQwS`) with their count, the bar and the latest
//! arrivals, and *Team settings* (`MH4Iy`). The bar is real: what was stored of what the server said, and no bar until
//! it said. The sync does not read the team settings yet, so their row says that rather than *after the tasks*
//! (`DownloadProgress::team_settings`). A failure shows in the tasks' row, with *Try again*.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::sharing::{
  Connection, RemoteMatch,
  project::{Arrival, DownloadProgress, DownloadState, PartState},
  remote_match,
};

use super::read::{Linked, Origin};
use crate::{
  screens::settings::orchester::shown_address,
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, IconSize, StartupText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    status::workflow_chip,
    tooltip::Truncated,
  },
};

/// The bar and the arrivals at their widest (`IQJ5E`, `Cwr2i`).
const TASKS_WIDTH: f32 = 520.0;
/// The bar's height (`IQJ5E`).
const BAR_HEIGHT: f32 = 6.0;
/// An arrival's key (`M0Kt9C`).
const KEY_WIDTH: f32 = 64.0;

/// How a part stands: its mark and whether its title is `text_muted`.
#[derive(Clone, Copy)]
enum Mark {
  Done,
  Working,
  Attention,
  Failed,
  Waiting,
}

impl Mark {
  fn element(self) -> Element {
    let (glyph, color) = match self {
      Self::Done => (Icon::CircleCheck, PaletteColor::Success),
      Self::Working => (Icon::LoaderCircle, PaletteColor::TextPrimary),
      Self::Attention => (Icon::CircleAlert, PaletteColor::Warning),
      Self::Failed => (Icon::CircleAlert, PaletteColor::Danger),
      Self::Waiting => (Icon::CircleDashed, PaletteColor::TextMuted),
    };
    icon(glyph, IconSize::Lg, color).into()
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Done => "done",
      Self::Working => "working",
      Self::Attention => "attention",
      Self::Failed => "failed",
      Self::Waiting => "waiting",
    }
  }
}

/// One row of the box.
struct Part {
  id: String,
  mark: Mark,
  title: String,
  sub: String,
  /// Under the line under its title.
  below: Vec<Element>,
}

/// The box of `linked`'s parts, whose ids start with `id`.
pub(super) fn list(
  ctx: &mut Ctx,
  id: &str,
  linked: &Linked,
  (origin, progress): (Option<&Origin>, Option<&DownloadProgress>),
) -> Element {
  let parts = [
    joined(ctx, id, linked),
    linked_part(ctx, id, linked, origin),
    tasks(ctx, id, linked, progress),
    team_settings(ctx, id, progress),
  ];
  let count = parts.len();
  Column::new()
    .id(format!("{id}-parts"))
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(
      parts
        .into_iter()
        .enumerate()
        .map(|(index, part)| row(part, index + 1 == count)),
    )
    .into()
}

fn row(part: Part, last: bool) -> Row {
  let Part {
    id,
    mark,
    title,
    sub,
    below,
  } = part;
  let title = Text::new(&title)
    .id(format!("{id}-title"))
    .variant(StartupText::RowTitle.role())
    .width(Dimension::Pct(100.0));
  let title = if matches!(mark, Mark::Waiting) {
    title.color(PaletteColor::TextMuted)
  } else {
    title
  };
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(title)
    .child(
      Text::new(&sub)
        .id(format!("{id}-sub"))
        .variant(StartupText::Sub.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(below);
  let row = Row::new()
    .id(id)
    .describe("state", mark.name())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(mark.element())
    .child(text);
  if last {
    row
  } else {
    row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  }
}

/// *Joined Acme*, or that this computer has no sign-in there, or no such server any more.
fn joined(ctx: &Ctx, id: &str, linked: &Linked) -> Part {
  let id = format!("{id}-joined");
  let sharing = &linked.sharing;
  let Some((server, address)) = sharing.server.as_ref() else {
    return Part {
      id,
      mark: Mark::Attention,
      title: ctx.t("startup.shared.download.joined.noServerTitle").to_string(),
      sub: ctx.t("startup.shared.download.joined.noServerSub").to_string(),
      below: Vec::new(),
    };
  };
  let names = [
    ("server", server.as_str()),
    ("address", shown_address(address.as_str())),
  ];
  if matches!(sharing.connection, Some(Connection::SignedOut) | None) {
    return Part {
      id,
      mark: Mark::Attention,
      title: ctx
        .t_args("startup.shared.download.joined.signedOutTitle", names)
        .to_string(),
      sub: ctx
        .t_args("startup.shared.download.joined.signedOutSub", names)
        .to_string(),
      below: Vec::new(),
    };
  }
  Part {
    id,
    mark: Mark::Done,
    title: ctx.t_args("startup.shared.download.joined.title", names).to_string(),
    sub: ctx.t_args("startup.shared.download.joined.sub", names).to_string(),
    below: Vec::new(),
  }
}

/// *Linked to ~/code/kontur*, and how its `origin` compares with what the server names, once both are read.
fn linked_part(ctx: &Ctx, id: &str, linked: &Linked, origin: Option<&Origin>) -> Part {
  let folder = linked.root.display().to_string();
  let sub = origin.map_or_else(String::new, |Origin(origin)| origin_words(ctx, linked, origin.as_ref()));
  Part {
    id: format!("{id}-linked"),
    mark: Mark::Done,
    title: ctx
      .t_args("startup.shared.download.linked.title", [("folder", folder.as_str())])
      .to_string(),
    sub,
    below: Vec::new(),
  }
}

fn origin_words(ctx: &Ctx, linked: &Linked, origin: Option<&orchester_core::repository::RepositoryIdentity>) -> String {
  let Some(checkout) = origin else {
    return ctx.t("startup.shared.download.linked.noOrigin").to_string();
  };
  let named = [("origin", checkout.as_str())];
  let Some(hint) = linked.hint.as_ref() else {
    return ctx.t_args("startup.shared.download.linked.origin", named).to_string();
  };
  match remote_match(origin, hint.as_ref()) {
    RemoteMatch::Same => ctx.t_args("startup.shared.download.linked.matches", named),
    RemoteMatch::NoHint | RemoteMatch::NoOrigin => ctx.t_args("startup.shared.download.linked.noHint", named),
    RemoteMatch::Different { checkout, hint } => ctx.t_args(
      "startup.shared.download.linked.differs",
      [("origin", checkout.as_str()), ("hint", hint.as_str())],
    ),
  }
  .to_string()
}

/// The tasks: how far they came, the bar, why they stopped and the latest arrivals.
fn tasks(ctx: &mut Ctx, id: &str, linked: &Linked, progress: Option<&DownloadProgress>) -> Part {
  let id = format!("{id}-tasks");
  let server = linked.sharing.server_name();
  let Some(progress) = progress else {
    return Part {
      id,
      mark: Mark::Waiting,
      title: ctx.t("startup.shared.download.tasks.idle").to_string(),
      sub: ctx
        .t_args("startup.shared.download.tasks.idleSub", [("server", server)])
        .to_string(),
      below: Vec::new(),
    };
  };
  let (mark, key) = match &progress.state {
    DownloadState::Downloading => (Mark::Working, "startup.shared.download.tasks.downloading"),
    DownloadState::Done => (Mark::Done, "startup.shared.download.tasks.done"),
    DownloadState::Failed { .. } => (Mark::Failed, "startup.shared.download.tasks.failed"),
  };
  let mut below: Vec<Element> = Vec::new();
  if let (Some(total), false) = (progress.total, progress.state == DownloadState::Done) {
    below.push(bar(&id, progress.stored, total).into());
  }
  if let DownloadState::Failed { reason, .. } = &progress.state {
    let reason = message_text(ctx, reason);
    below.push(failure(ctx, &id, linked, &reason));
  }
  if !progress.latest.is_empty() {
    below.push(arrivals(ctx, &id, &progress.latest));
  }
  Part {
    sub: count(ctx, progress),
    title: ctx.t(key).to_string(),
    id,
    mark,
    below,
  }
}

/// *84 of 212*; before the server said how many, how many so far, and before the first of them, that the server is
/// being asked, only while it is: a download that stopped there asks nobody.
fn count(ctx: &Ctx, progress: &DownloadProgress) -> String {
  let stored = progress.stored;
  match (progress.total, stored) {
    (Some(total), _) => ctx.t_args(
      "startup.shared.download.tasks.count",
      [("stored", stored.to_string()), ("total", total.to_string())],
    ),
    (None, 0) if progress.state == DownloadState::Downloading => ctx.t_args(
      "startup.shared.download.tasks.asking",
      [("server", progress.server.as_str())],
    ),
    (None, 0) => ctx.t("startup.shared.download.tasks.none"),
    (None, 1) => ctx.t("startup.shared.download.tasks.soFar.one"),
    (None, count) => ctx.t_args(
      "startup.shared.download.tasks.soFar.other",
      [("count", count.to_string())],
    ),
  }
  .to_string()
}

/// `stored` of `total` as a bar (`IQJ5E`) of `surface_input`, filled with `accent`.
fn bar(id: &str, stored: usize, total: usize) -> Row {
  let done = if total == 0 {
    100.0
  } else {
    stored.min(total) as f32 * 100.0 / total as f32
  };
  Row::new()
    .id(format!("{id}-bar"))
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(TASKS_WIDTH))
    .height(Dimension::Px(BAR_HEIGHT))
    .background(PaletteColor::SurfaceInput)
    .rounded(ExtraRadius::Full.role())
    .clip()
    .child(
      Row::new()
        .id(format!("{id}-bar-done"))
        .width(Dimension::Pct(done))
        .height(Dimension::Px(BAR_HEIGHT))
        .background(PaletteColor::Accent)
        .rounded(ExtraRadius::Full.role()),
    )
}

/// Why the download stopped, and *Try again*, which goes on from the page that failed.
fn failure(ctx: &mut Ctx, id: &str, linked: &Linked, reason: &str) -> Element {
  let (servers, root) = (Services::of(ctx).servers, linked.root.clone());
  Column::new()
    .id(format!("{id}-failure"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(field_error(&format!("{id}-reason"), reason))
    .child(
      ActionButton::new(format!("{id}-retry"), ButtonKind::Secondary, ctx.t("common.tryAgain"))
        .icon(Icon::RefreshCw)
        .on_click(move || servers.retry_download(&root)),
    )
    .into()
}

/// The latest tasks to arrive, newest first (`Arrived` `Cwr2i`).
fn arrivals(ctx: &mut Ctx, id: &str, latest: &[Arrival]) -> Element {
  let mut column = Column::new()
    .id(format!("{id}-arrived"))
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(TASKS_WIDTH));
  for arrival in latest {
    let id = format!("{id}-arrival-{}", arrival.key);
    let title = Truncated::new(
      format!("{id}-title"),
      arrival.title.clone(),
      StartupText::ArrivalTitle.role(),
    )
    .render(ctx);
    column = column.child(
      Row::new()
        .id(id.clone())
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(ExtraSpacing::Snug.role())
            .bottom(ExtraSpacing::Snug.role()),
        )
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(
          Text::new(&arrival.key)
            .id(format!("{id}-key"))
            .variant(StartupText::ArrivalKey.role())
            .width(Dimension::Px(KEY_WIDTH))
            .nowrap(),
        )
        .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(title))
        .child(workflow_chip(ctx, &format!("{id}-chip"), arrival.workflow)),
    );
  }
  column.into()
}

/// *Team settings*, as the download reports them.
fn team_settings(ctx: &Ctx, id: &str, progress: Option<&DownloadProgress>) -> Part {
  let state = progress.map_or(PartState::NotAvailable, |progress| progress.team_settings);
  let (mark, key) = match state {
    PartState::Pending => (Mark::Waiting, "startup.shared.download.settings.pending"),
    PartState::Done => (Mark::Done, "startup.shared.download.settings.done"),
    PartState::NotAvailable => (Mark::Waiting, "startup.shared.download.settings.notAvailable"),
  };
  Part {
    id: format!("{id}-settings"),
    mark,
    title: ctx.t("startup.shared.download.settings.title").to_string(),
    sub: ctx.t(key).to_string(),
    below: Vec::new(),
  }
}
