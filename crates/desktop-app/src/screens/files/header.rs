//! The page's header (`File header` `TpYaM`, 32 from the sides, 16 over and 14 under, a `border` line under
//! it): the crumbs (the project, *Files*, the file's folders in Geist Mono), the title row (an 18 icon, the
//! file's name in 20/600, then *Go to file…*, the version switch, *Copy lines* or *Copy file* and *Open in
//! editor*) and a line saying where the file was read and what it is ("Run #12 worktree · ORC-24 · Rust ·
//! 612 lines · 21.4 KB"), with why *Open in editor* is unavailable at its end, under the button (`Actions`
//! `mQeh3`). A path the page refuses is its title in Geist Mono with a `ban` icon (`S3Py8v`);
//! with no file the title is *Files* (`o3iVXj`); a version that no longer exists says so in `warning`
//! (`SyXwU`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display::byte_size,
  files::{FilePath, FileView, OpenFacts, Platform, Version, open_with},
  highlight::language,
};

use super::{Page, actions, goto, reads::Shown, version};
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

/// The title row's height (`a5LZb`).
const TITLE_HEIGHT: f32 = 28.0;

pub fn header(ctx: &mut Ctx, page: &Page<'_>, gone: bool) -> Element {
  let (switch, menu) = version::switch(ctx, page);
  let title = Row::new()
    .id("files-title-row")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TITLE_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(title(ctx, page))
    .child(Row::new().flex(1.0))
    .child(goto::trigger(ctx, page))
    .child(switch)
    .with_children(copy(ctx, page, gone))
    .with_children(open(ctx, page, gone));
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
    .child(meta(ctx, page, gone))
    .with_children(menu)
    .into()
}

/// The project, *Files* and the file's folders, a folder opening in the tree; with no file, the project alone
/// (`o3iVXj`).
fn crumbs(ctx: &mut Ctx, page: &Page<'_>) -> Row {
  let navigation = Navigation::of(ctx);
  let project = page.project.open.project.id();
  let overview = navigation.clone();
  let mut crumbs = vec![Crumb::link(
    page.project.open.project.name().as_str().to_owned(),
    move || {
      overview.go(Route::Project(project, ProjectPage::Overview(OverviewTab::default())));
    },
  )];
  if page.route.file().is_some() {
    let root = page.route_to(None, page.at().clone());
    crumbs.push(Crumb::link(ctx.t("files.title"), move || navigation.go(root.clone())));
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
        .mono(),
      );
    }
  }
  breadcrumb(ctx, "files-crumbs", CrumbSeparator::Slash, crumbs)
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
    .nowrap();
  let text = if mono {
    text.variant(FilesText::RefusedPath.role())
  } else {
    text
  };
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Xl, PaletteColor::TextSecondary))
    .child(text)
}

/// Where the file was read and what it is, or that the version no longer exists; and what a press the
/// page refused was.
fn meta(ctx: &Ctx, page: &Page<'_>, gone: bool) -> Row {
  let (glyph, color, text) = if gone {
    (
      Icon::TriangleAlert,
      PaletteColor::Warning,
      ctx.t("files.gone.meta").to_string(),
    )
  } else {
    (version::glyph(page.at()), PaletteColor::TextMuted, meta_text(ctx, page))
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
  let unavailable = page
    .file()
    .filter(|_| shows_open(page, gone))
    .and_then(|file| open_unavailable(page, file))
    .map(|key| {
      Text::new(&ctx.t(key))
        .id("files-open-reason")
        .variant(TypographyStyle::Caption)
        .describe("role", "unavailable-reason")
        .nowrap()
    });
  Row::new()
    .id("files-meta")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(
      Text::new(&text)
        .id("files-meta-text")
        .variant(ExtraTypography::Note.role())
        .color(color)
        .nowrap(),
    )
    .with_children(refusal)
    .with_children(stale)
    .child(Row::new().flex(1.0))
    .with_children(unavailable)
}

/// "Run #12 worktree · ORC-24 · Rust · 612 lines · 21.4 KB".
fn meta_text(ctx: &Ctx, page: &Page<'_>) -> String {
  let mut parts = vec![version::label(ctx, page)];
  let choice = page.shown_choice();
  match page.at() {
    Version::Checkout => {
      parts.push(page.project.open.repo_root.display().to_string());
      parts.extend(
        choice
          .and_then(|choice| choice.branch.as_ref())
          .map(ToString::to_string),
      );
    }
    Version::Run(_) => parts.extend(page.run_facts().map(|run| run.task.to_string())),
    Version::Base | Version::Revision(_) => {
      parts.extend(
        choice
          .and_then(|choice| choice.branch.as_ref())
          .map(ToString::to_string),
      );
    }
  }
  if let Some(Shown::Read(read)) = &page.read {
    parts.extend(file_parts(ctx, page, read));
  }
  parts.join(&ctx.t("files.meta.separator"))
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
/// disk ([`orchester_workspace::files::open_target`]). A file too large to show opens by its kind.
fn open_refusal(page: &Page<'_>, file: &FilePath) -> Option<&'static str> {
  let Some(platform) = Platform::current() else {
    return Some("files.openUnsupported");
  };
  let text = !matches!(page.read, Some(Shown::Read(FileView::Binary { .. })));
  let facts = OpenFacts {
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
  let button = ActionButton::new("files-open", ButtonKind::Secondary, ctx.t("files.openInEditor"))
    .icon(Icon::SquareArrowOutUpRight);
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
