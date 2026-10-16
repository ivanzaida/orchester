//! The page's header (`File header` `TpYaM`, 32 from the sides, 16 over and 14 under, a `border` line under
//! it): the crumbs (the project, *Files*, the file's folders in Geist Mono), the title row (an 18 icon, the
//! file's name in 20/600, then *Go to file…*, the version switch, *Copy lines* or *Copy file* and *Open in
//! editor*) and a line saying where the file was read and what it is ("Run #12 worktree · ORC-24 · Rust ·
//! 612 lines · 21.4 KB"). As the Tasks header (`mQeh3`) lays it out, the title and the meta line are a column on
//! the left, the meta cut with an ellipsis when it is long, and the actions a column on the right with why *Open in
//! editor* is unavailable under them, at their end: the reason sits beside the meta line, so the header keeps its
//! height, and a long reason wraps inside its column. The actions keep their width; when that would leave the title
//! less than [`TITLE_MIN_WIDTH`] (a narrow window), they move to a row of their own under the meta line, at its end,
//! so the file stays named and every button stays in the header ([`Room`]). The title and the meta line give way,
//! the checkout's folder first, shortened in its middle (`H:\…\work\kontur`), so the branch and the
//! file's facts after it stay in view. Every crumb is an ancestor of the file, all in
//! `text_muted`. A path the page refuses is its title in Geist Mono with a `ban` icon (`S3Py8v`);
//! with no file the title is *Files* (`o3iVXj`); a version that no longer exists says so in `warning`
//! (`SyXwU`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display::{byte_size, shorten_path},
  files::{FilePath, FileView, OpenFacts, Platform, Version, open_with},
  highlight::language,
};

use super::{Page, actions, goto, reads::Shown, states, version};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, FilesText, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    icon::{Icon, file_icon, icon},
    message::message_text,
  },
};

/// The heights of the crumbs (`gxF9b`), the title row (`a5LZb`) and the meta line (`F0BHDL`).
const CRUMBS_HEIGHT: f32 = 16.0;
const TITLE_HEIGHT: f32 = 28.0;
const META_HEIGHT: f32 = 15.0;
/// How much faster than the rest of the meta line the checkout's folder gives way when the line is short.
const PATH_GIVES_WAY: f32 = 100.0;
/// The least room the title and the meta line keep beside the actions, enough for a file's name.
const TITLE_MIN_WIDTH: f32 = 240.0;
/// The gap between the title column and the actions (`SpacingSize::Lg`).
const ACTIONS_GAP: f32 = 16.0;

/// The header's title row and its actions as the last layout measured them; zero before the first.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Room {
  row: f32,
  actions: f32,
}

impl Room {
  /// The room of a title row `row` wide beside actions `actions` wide, to whole pixels so that a layout that
  /// moves by a fraction measures the same room.
  pub fn new(row: f32, actions: f32) -> Self {
    Self {
      row: row.round(),
      actions: actions.round(),
    }
  }

  /// Whether the actions leave the title too little room beside them, so they go under it.
  fn actions_below(self) -> bool {
    self.row > 0.0 && self.row - self.actions - ACTIONS_GAP < TITLE_MIN_WIDTH
  }
}

impl lurq::app::component::DevtoolsInspectable for Room {}

pub fn header(ctx: &mut Ctx, page: &Page<'_>, gone: bool) -> Element {
  let (switch, menu) = version::switch(ctx, page);
  let buttons = Row::new()
    .id("files-actions")
    .height(Dimension::Px(TITLE_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(goto::trigger(ctx, page))
    .child(switch)
    .with_children(copy(ctx, page, gone))
    .with_children(open(ctx, page, gone));
  let reason = page
    .file()
    .filter(|_| shows_open(page, gone))
    .and_then(|file| open_unavailable(page, file));
  let reason = reason.map(|key| {
    Text::new(&ctx.t(key))
      .id("files-open-reason")
      .variant(TypographyStyle::Caption)
      .text_align(TextAlign::Right)
      .width(Dimension::Pct(100.0))
      .describe("role", "unavailable-reason")
  });
  let actions = Column::new()
    .id("files-actions-column")
    .ref_element(page.view.actions.clone())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(buttons)
    .with_children(reason);
  let named = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .child(title(ctx, page).height(Dimension::Px(TITLE_HEIGHT)))
    .child(meta(ctx, page, gone));
  let below = page.view.header_room.get().actions_below();
  let (beside, under) = if below {
    let under = Row::new()
      .id("files-actions-row")
      .width(Dimension::Pct(100.0))
      .justify(Justify::End)
      .child(actions);
    (None, Some(under))
  } else {
    (Some(actions), None)
  };
  let title = Row::new()
    .id("files-title-row")
    .ref_element(page.view.title_row.clone())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .describe("actions", if below { "below" } else { "beside" })
    .child(named)
    .with_children(beside);
  Column::new()
    .id("files-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Lg)
        .bottom(ExtraSpacing::Inset.role())
        .left(SpacingSize::Section)
        .right(SpacingSize::Section),
    )
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(crumbs(ctx, page))
    .child(title)
    .with_children(under)
    .with_children(menu)
    .into()
}

/// The project, *Files* and the file's folders, a folder opening in the tree; with no file, the project alone
/// (`o3iVXj`).
fn crumbs(ctx: &mut Ctx, page: &Page<'_>) -> Row {
  let navigation = Navigation::of(ctx);
  let project = page.project.open.project.id();
  let overview = navigation.clone();
  let mut crumbs = vec![
    Crumb::link(page.project.open.project.name().as_str().to_owned(), move || {
      overview.go(Route::Project(project, ProjectPage::Overview(OverviewTab::default())));
    })
    .ancestor(),
  ];
  if page.route.file().is_some() {
    let root = page.route_to(None, page.at().clone());
    crumbs.push(Crumb::link(ctx.t("files.title"), move || navigation.go(root.clone())).ancestor());
  }
  if let Some(file) = page.file() {
    for folder in file.folders() {
      let name = folder.rsplit('/').next().unwrap_or(folder).to_owned();
      let (open, folder) = (page.view.open.clone(), folder.to_owned());
      crumbs.push(
        Crumb::link(name, move || {
          let folders = folder
            .match_indices('/')
            .map(|(at, _)| &folder[..at])
            .chain([folder.as_str()]);
          open.set(open.get_untracked().with(folders));
        })
        .mono()
        .ancestor(),
      );
    }
  }
  breadcrumb(ctx, "files-crumbs", CrumbSeparator::Slash, crumbs).height(Dimension::Px(CRUMBS_HEIGHT))
}

/// The icon and the name: the file's, the path refused, or *Files*.
fn title(ctx: &Ctx, page: &Page<'_>) -> Row {
  let (glyph, text, mono) = match (&page.path, page.route.file()) {
    (Some(Ok(file)), _) => (file_icon(file.as_str()), file.name().to_owned(), false),
    (Some(Err(_)), Some(raw)) => (Icon::Ban, raw.to_owned(), true),
    _ => (Icon::FolderTree, ctx.t("files.title").to_string(), false),
  };
  let text = Text::new(&text)
    .id("files-title")
    .variant(TypographyStyle::Heading)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0));
  let text = if mono {
    text.variant(FilesText::RefusedPath.role())
  } else {
    text
  };
  Row::new()
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Xl, PaletteColor::TextSecondary).flex_shrink(0.0))
    .child(text)
}

/// Where the file was read and what it is, or that the version no longer exists; and what a press the
/// page refused was.
fn meta(ctx: &Ctx, page: &Page<'_>, gone: bool) -> Row {
  let (glyph, color, line) = if gone {
    (
      Icon::TriangleAlert,
      PaletteColor::Warning,
      MetaLine::whole(states::gone_meta(ctx, page)),
    )
  } else {
    (version::glyph(page.at()), PaletteColor::TextMuted, meta_line(ctx, page))
  };
  let refusal = page.view.refusal.get().map(|refusal| {
    Text::new(&ctx.t(refusal.key()))
      .id("files-refusal")
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::Warning)
      .nowrap()
  });
  let stale = page.stale.as_ref().map(|failure| {
    let reason = message_text(ctx, &failure.message());
    Text::new(&ctx.t_args("files.stale", [("reason", &*reason)]))
      .id("files-stale")
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::Warning)
      .nowrap()
  });
  Row::new()
    .id("files-meta")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(META_HEIGHT))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color.clone()).flex_shrink(0.0))
    .child(line.render(&color))
    .with_children(refusal)
    .with_children(stale)
}

/// The meta line in its parts: what comes before the checkout's folder, the folder, and what comes after it.
struct MetaLine {
  before: String,
  folder: Option<String>,
  after: String,
}

impl MetaLine {
  /// A line without a folder in it.
  fn whole(text: String) -> Self {
    Self {
      before: text,
      folder: None,
      after: String::new(),
    }
  }

  /// The line's texts in `color`, each cut with an ellipsis when the line is short, the folder long before the
  /// rest. Its parts stand a word space apart, as the separators' own spaces would.
  fn render(self, color: &PaletteColor) -> Row {
    let text = |words: &str, gives_way: f32| {
      Text::new(words)
        .variant(ExtraTypography::Note.role())
        .color(color.clone())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(gives_way)
        .min_width(Dimension::Px(0.0))
    };
    Row::new()
      .id("files-meta-text")
      .min_width(Dimension::Px(0.0))
      .flex_shrink(1.0)
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(text(&self.before, 1.0))
      .with_children(
        self
          .folder
          .map(|folder| text(&folder, PATH_GIVES_WAY).id("files-meta-folder")),
      )
      .with_children((!self.after.is_empty()).then(|| text(&self.after, 1.0).id("files-meta-facts")))
  }
}

/// "Run #12 worktree · ORC-24 · Implement · Rust · 612 lines · 21.4 KB"; for the checkout its folder, shortened in
/// its middle, after the version's name.
fn meta_line(ctx: &Ctx, page: &Page<'_>) -> MetaLine {
  let separator = ctx.t("files.meta.separator");
  let mut parts = Vec::new();
  let choice = page.shown_choice();
  let branch = || {
    choice
      .and_then(|choice| choice.branch.as_ref())
      .map(ToString::to_string)
  };
  let folder = match page.at() {
    Version::Checkout => {
      parts.extend(branch());
      Some(shorten_path(&page.project.open.repo_root, version::FOLDER_CHARS))
    }
    Version::Run(_) => {
      parts.extend(page.run_facts().into_iter().flat_map(version::run_parts));
      None
    }
    Version::Base | Version::Revision(_) => {
      parts.extend(branch());
      None
    }
  };
  if let Some(Shown::Read(read)) = &page.read {
    parts.extend(file_parts(ctx, page, read));
  }
  let label = version::label(ctx, page);
  let trimmed = separator.trim();
  match folder {
    Some(folder) => MetaLine {
      before: format!("{label} {trimmed}"),
      folder: Some(folder),
      after: parts
        .iter()
        .map(|part| format!("{trimmed} {part}"))
        .collect::<Vec<_>>()
        .join(" "),
    },
    None => MetaLine::whole([label].into_iter().chain(parts).collect::<Vec<_>>().join(&separator)),
  }
}

/// The file's language, lines and size; or that it is binary or text, and its size.
fn file_parts(ctx: &Ctx, page: &Page<'_>, read: &FileView) -> Vec<String> {
  match read {
    FileView::Text(text) => {
      let mut parts: Vec<String> = page
        .file()
        .and_then(|file| language(file.as_str()))
        .map(str::to_owned)
        .into_iter()
        .collect();
      parts.push(version::lines_text(ctx, text.lines().len()));
      parts.push(message_text(ctx, &byte_size(text.size())).to_string());
      parts
    }
    FileView::Binary { size } => vec![
      ctx.t("files.meta.binary").to_string(),
      message_text(ctx, &byte_size(*size)).to_string(),
    ],
    FileView::TooLarge { size } => vec![
      ctx.t("files.meta.text").to_string(),
      message_text(ctx, &byte_size(*size)).to_string(),
    ],
  }
}

/// *Copy lines* while lines are selected, else *Copy file*; only for a text file.
fn copy(ctx: &mut Ctx, page: &Page<'_>, gone: bool) -> Option<Element> {
  let Some(Shown::Read(FileView::Text(text))) = &page.read else {
    return None;
  };
  if gone {
    return None;
  }
  let lines = page.route.line();
  let key = if lines.is_some() {
    "files.copyLines"
  } else {
    "files.copyFile"
  };
  let clipboard = Copier::of(ctx);
  let press = actions::copy(clipboard, page.view.refusal.clone(), (text.clone(), lines));
  Some(
    ActionButton::new("files-copy", ButtonKind::Ghost, ctx.t(key))
      .icon(Icon::Copy)
      .on_click(press)
      .into(),
  )
}

/// Why *Open in editor* cannot open `file` as the page read it, before it is pressed; the press checks again on
/// disk ([`orchester_workspace::files::open_target`]). A file too large to show opens by its kind. The path weighed
/// is the file's in the project's folder, which holds the runs' worktrees too.
fn open_refusal(page: &Page<'_>, file: &FilePath) -> Option<&'static str> {
  let Some(platform) = Platform::current() else {
    return Some("files.openUnsupported");
  };
  let text = !matches!(page.read, Some(Shown::Read(FileView::Binary { .. })));
  let path = page.project.open.repo_root.join(file.as_str());
  let facts = OpenFacts {
    path: &path.to_string_lossy(),
    name: file.name(),
    text,
    link: false,
    executable: false,
  };
  open_with(platform, &facts).err().map(actions::refusal_key)
}

/// Whether the header has *Open in editor*: for a file read, while its version exists.
fn shows_open(page: &Page<'_>, gone: bool) -> bool {
  !gone && !matches!(page.read, Some(Shown::Failed(_)))
}

/// Why *Open in editor* is unavailable for `file`: at a commit, whose file is not on disk, or what
/// [`open_refusal`] says.
fn open_unavailable(page: &Page<'_>, file: &FilePath) -> Option<&'static str> {
  if matches!(page.at(), Version::Base | Version::Revision(_)) {
    return Some("files.openAtCommit");
  }
  open_refusal(page, file)
}

/// *Open in editor*, for a file the page opens; unavailable, with the reason at the end of the line under it,
/// when [`open_unavailable`] says so.
fn open(ctx: &mut Ctx, page: &Page<'_>, gone: bool) -> Option<Element> {
  let file = page.file()?.clone();
  if !shows_open(page, gone) {
    return None;
  }
  let button =
    ActionButton::new("files-open", ButtonKind::Secondary, ctx.t("files.openInEditor")).icon(Icon::ExternalLink);
  if let Some(reason) = open_unavailable(page, &file) {
    return Some(button.unavailable(ctx.t(reason)).into());
  }
  let programs = Services::of(ctx).programs;
  let press = actions::open_in_editor(
    programs,
    page.view.refusal.clone(),
    (page.project.open.repo_root.clone(), page.at().clone(), file),
  );
  Some(button.on_click(press).into())
}
