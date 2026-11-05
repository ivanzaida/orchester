//! The files a Test step's suites left, as the Tests card offers them (`qN2FK`): a failed test's links, one per kind
//! of file it has (*Screenshot*, *Trace*: 12/500 `text_secondary`, `Hae0W`), and *Artifacts* at the card's end
//! (`gVlMK`), a group per kind in a row 8 apart (`eKzJL`), each on `surface_raised`, radius 6, 8 over and under and 10
//! at its sides: a 14 `text_secondary` glyph, its name in 12.5/500, how many in 12 `text_muted`, and a 13 `text_muted`
//! `arrow-up-right` at its end (`VzlTw`). A press hands the file to its default program, or the folder holding a
//! group's files; a file that could run as a program opens its folder instead
//! ([`orchester_core::test_step::ArtifactKind::opens_itself`]).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::test_step::{ArtifactKind, TestStepRecord};

use super::words::counted;
use crate::{
  services::{Services, links::Links},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, TestsText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The glyph, the i18n key of a group's name and of a test's link, for files of `kind`. The design's `image` and
/// `file-archive` have no glyph in the bundled `lucide.ttf`: `file-image` and `archive` draw them.
const fn look(kind: ArtifactKind) -> (Icon, &'static str, &'static str) {
  match kind {
    ArtifactKind::Screenshot => (
      Icon::FileImage,
      "summary.tests.artifacts.screenshots",
      "summary.tests.artifacts.screenshot",
    ),
    ArtifactKind::Trace => (
      Icon::Archive,
      "summary.tests.artifacts.traces",
      "summary.tests.artifacts.trace",
    ),
    ArtifactKind::Video => (
      Icon::MonitorPlay,
      "summary.tests.artifacts.videos",
      "summary.tests.artifacts.video",
    ),
    ArtifactKind::Log => (
      Icon::ScrollText,
      "summary.tests.artifacts.logs",
      "summary.tests.artifacts.log",
    ),
    ArtifactKind::Other => (
      Icon::File,
      "summary.tests.artifacts.others",
      "summary.tests.artifacts.other",
    ),
  }
}

/// What a press on a file of `kind` at `stored` opens: the file, or the folder holding it.
fn target(stored: &str, kind: ArtifactKind) -> PathBuf {
  let path = Path::new(stored);
  if kind.opens_itself() {
    return path.to_path_buf();
  }
  path.parent().map_or_else(|| path.to_path_buf(), Path::to_path_buf)
}

/// A press that opens `path` through the session's `links`.
fn open(links: &Links, path: PathBuf) -> impl Fn() + Send + Sync + 'static {
  let links = links.clone();
  move || {
    if let Err(error) = links.open_file(&path) {
      tracing::warn!(%error, path = %path.display(), "a Test step's artifact was not opened");
    }
  }
}

/// The links of a failed test whose report attached `artifacts`: the first file of each kind, in the kinds' order.
pub fn links(ctx: &mut Ctx, id: &str, artifacts: &[String]) -> Vec<Element> {
  let links = Services::of(ctx).links;
  ArtifactKind::ALL
    .into_iter()
    .filter_map(|kind| {
      let stored = artifacts.iter().find(|stored| ArtifactKind::of(stored) == kind)?;
      let (_, _, word) = look(kind);
      let word = ctx.t(word);
      let link = pressable(open(&links, target(stored, kind)))
        .id(format!("{id}-artifact-{}", kind_name(kind)))
        .describe(
          "a11y_name",
          ctx.t_args("summary.tests.artifacts.open", [("name", &*word)]),
        )
        .describe("path", stored.clone())
        .padding(Padding::symmetric(
          ExtraSpacing::Tight.role(),
          ExtraSpacing::Tight.role(),
        ))
        .rounded(RadiusSize::Sm)
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .child(
          Text::new(&word)
            .variant(TypographyStyle::Label)
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        );
      Some(link.into())
    })
    .collect()
}

/// *Artifacts* with a group per kind of file the suites left; `None` when they left none.
pub fn section(ctx: &mut Ctx, record: &TestStepRecord) -> Option<Element> {
  let links = Services::of(ctx).links;
  let groups: Vec<Element> = ArtifactKind::ALL
    .into_iter()
    .filter_map(|kind| {
      let files: Vec<(&str, bool)> = record
        .artifacts()
        .filter(|artifact| ArtifactKind::of(&artifact.stored) == kind)
        .map(|artifact| (artifact.stored.as_str(), artifact.test.is_some()))
        .collect();
      (!files.is_empty()).then(|| group(ctx, &links, kind, &files))
    })
    .collect();
  if groups.is_empty() {
    return None;
  }
  let section = Column::new()
    .id("task-tests-artifacts")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .left(ExtraSpacing::Inset.role())
        .right(ExtraSpacing::Inset.role())
        .bottom(ExtraSpacing::Inset.role()),
    )
    .spacing(SpacingSize::Sm)
    .border_top(Border::inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("summary.tests.artifacts.title")).variant(TestsText::Heading.role()))
    .child(
      Row::new()
        .id("task-tests-artifact-groups")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .with_children(groups),
    );
  Some(section.into())
}

/// The group of `files` of `kind`, each with whether a test's report attached it; a press opens the folder that
/// holds them all.
fn group(ctx: &Ctx, links: &Links, kind: ArtifactKind, files: &[(&str, bool)]) -> Element {
  let (glyph, name, _) = look(kind);
  let id = format!("task-tests-artifacts-{}", kind_name(kind));
  let count = if files.iter().all(|(_, attached)| *attached) {
    ctx.t_args(
      "summary.tests.artifacts.atFailures",
      [("count", files.len().to_string())],
    )
  } else {
    counted(
      ctx,
      (
        "summary.tests.artifacts.files.one",
        "summary.tests.artifacts.files.other",
      ),
      files.len(),
    )
  };
  let folder = common_folder(files.iter().map(|(stored, _)| Path::new(*stored)));
  let name: Arc<str> = ctx.t(name);
  pressable(open(links, folder.clone()))
    .id(id.clone())
    .describe(
      "a11y_name",
      ctx.t_args("summary.tests.artifacts.openGroup", [("name", &*name)]),
    )
    .describe("path", folder.to_string_lossy().into_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&name)
        .id(format!("{id}-name"))
        .variant(ExtraTypography::OptionTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&count)
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Base, PaletteColor::TextMuted))
    .into()
}

/// The deepest folder holding every one of `files`.
fn common_folder<'a>(files: impl Iterator<Item = &'a Path>) -> PathBuf {
  let mut common: Option<PathBuf> = None;
  for file in files {
    let folder = file.parent().unwrap_or(file);
    common = Some(match common {
      None => folder.to_path_buf(),
      Some(common) => common
        .components()
        .zip(folder.components())
        .take_while(|(left, right)| left == right)
        .map(|(part, _)| part)
        .collect(),
    });
  }
  common.unwrap_or_default()
}

/// `kind` in an element's id.
const fn kind_name(kind: ArtifactKind) -> &'static str {
  match kind {
    ArtifactKind::Screenshot => "screenshot",
    ArtifactKind::Trace => "trace",
    ArtifactKind::Video => "video",
    ArtifactKind::Log => "log",
    ArtifactKind::Other => "other",
  }
}
