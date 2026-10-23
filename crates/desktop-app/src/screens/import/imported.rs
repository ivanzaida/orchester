//! What a copied pipeline shows of the import that created it (`z3vhzo` in Import resources `Dzzks`): the strip
//! under the editor's header while it is not published (`Import strip` `rmn80`), and the inspector's Imported,
//! Imported with it and Missing sections (`diBqj`, `TyfNt`, `U5Vgf`). What it says comes from
//! `orchester_workspace::import::imported`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  import::{Content, Entity},
  time::Timestamp,
};
use orchester_workspace::import::{
  imported::{Came, Imported, Missing, WithIt},
  review::Subject,
};

use super::{words, work::page_of};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::{PropertyValue, note, property, section_title},
    plan_window,
    pressable::opening,
  },
};

/// A resource that came along with the copy (`Link · Build preview · Draft v1` `fnKda`, a 32 high `Used-by link`).
const LINK_HEIGHT: f32 = 32.0;
/// A section title's row with the line over it (`Title · Imported` `FB3ig` under its section's top padding).
const SECTION_TITLE_HEIGHT: f32 = 30.0;
/// A missing connection or secret (`Missing · PREVIEW_HOST_API_KEY` `mVFDI`).
const MISSING_HEIGHT: f32 = 24.0;

/// The strip over the copy just imported: where it came from, what the import did, and Open Tools while
/// something is left unresolved.
pub fn strip(ctx: &mut Ctx, project: ProjectId, imported: &Imported) -> Element {
  let tools = imported.needs_setup().then(|| {
    let open = Navigation::of(ctx).to(Route::Project(project, ProjectPage::Tools));
    ActionButton::new(
      "pipeline-import-open-tools",
      ButtonKind::Ghost,
      ctx.t("import.result.openTools"),
    )
    .icon(Icon::Wrench)
    .opens(open)
  });
  Row::new()
    .id("pipeline-import-strip")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Tabs.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::CircleCheck, IconSize::Nav, PaletteColor::Success))
    .child(
      Text::new(&ctx.t_args("import.result.title", [("project", imported.project.as_str())]))
        .id("pipeline-import-title")
        .variant(ExtraTypography::FormSection.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t_args("import.result.summary", [("totals", &*totals(ctx, imported))]))
        .id("pipeline-import-summary")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(tools)
    .into()
}

/// "3 drafts created · 2 local resources reused · 2 left unresolved".
fn totals(ctx: &Ctx, imported: &Imported) -> Arc<str> {
  let mut all = vec![words::counted(
    ctx,
    imported.created,
    "import.result.created.one",
    "import.result.created.other",
  )];
  if imported.reused > 0 {
    all.push(words::counted(
      ctx,
      imported.reused,
      "import.totals.reused.one",
      "import.totals.reused.other",
    ));
  }
  if imported.unresolved > 0 {
    all.push(words::counted(
      ctx,
      imported.unresolved,
      "import.totals.unresolved.one",
      "import.totals.unresolved.other",
    ));
  }
  words::parts(ctx, &all)
}

/// The inspector's sections on the import, in `target`, this project, by its name: Imported, then what came with
/// the copy and what is missing, when there is any.
pub fn sections(ctx: &mut Ctx, (project, target): (ProjectId, &str), imported: &Imported) -> Vec<Element> {
  let when = plan_window::when(
    ctx,
    imported.at,
    Timestamp::now(),
    ("import.result.today", "import.result.day"),
  );
  let source = match imported.version {
    Some(version) => ctx.t_args(
      "import.result.sourcePublished",
      [
        ("name", imported.source.clone()),
        ("version", version.get().to_string()),
      ],
    ),
    None => ctx.t_args("import.result.sourceDraft", [("name", imported.source.as_str())]),
  };
  let mut all: Vec<Element> = vec![
    title("pipeline-imported", &ctx.t("import.result.imported"), Some(&when)),
    property(
      "pipeline-imported-from",
      &ctx.t("import.result.from"),
      &imported.project,
      PropertyValue::Plain,
    )
    .into(),
    property(
      "pipeline-imported-source",
      &ctx.t("import.result.sourceVersion"),
      &source,
      PropertyValue::Plain,
    )
    .into(),
    note(
      "pipeline-imported-note",
      Icon::History,
      &ctx.t_args("import.result.independent", [("project", imported.project.as_str())]),
    )
    .into(),
  ];
  if !imported.with_it.is_empty() {
    let count = imported.with_it.len().to_string();
    all.push(title(
      "pipeline-imported-with",
      &ctx.t("import.result.withIt"),
      Some(&count),
    ));
    for came in &imported.with_it {
      all.push(link(ctx, project, came));
    }
  }
  if !imported.missing.is_empty() {
    let count = imported.missing.len().to_string();
    all.push(title(
      "pipeline-imported-missing",
      &ctx.t_args("import.result.missing", [("project", target)]),
      Some(&count),
    ));
    for (index, missing) in imported.missing.iter().enumerate() {
      all.push(missing_row(index, missing));
    }
    all.push(
      note(
        "pipeline-imported-missing-note",
        Icon::Info,
        &ctx.t_args("import.result.missingNote", [("project", target)]),
      )
      .into(),
    );
  }
  all
}

/// A section's title with the line over it and what it counts or when, at its end.
fn title(id: &str, title: &str, meta: Option<&str>) -> Element {
  section_title(id, title, meta)
    .height(Dimension::Px(SECTION_TITLE_HEIGHT))
    .padding(Padding::new().top(ExtraSpacing::Loose.role()))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .into()
}

/// A resource that came along with the copy, which opens its page.
fn link(ctx: &mut Ctx, project: ProjectId, came: &WithIt) -> Element {
  let label = link_label(ctx, came);
  let page = page_of(came.id, came.content.kind(), Some(&came.content));
  let open = Navigation::of(ctx).to(Route::Project(project, page));
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LINK_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph(&came.content), IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Base, PaletteColor::TextMuted));
  opening(open)
    .id(format!("pipeline-imported-link-{}", came.id))
    .width(Dimension::Pct(100.0))
    .describe("label", label.to_string())
    .child(row)
    .into()
}

fn glyph(content: &Content) -> Icon {
  words::glyph(Subject::of_content(content))
}

/// "Build preview · Draft v1", "Website preview · Draft v1 · renamed", "Builder v2 · reused".
fn link_label(ctx: &Ctx, came: &WithIt) -> Arc<str> {
  let name = ("name", came.name.clone());
  let key = match (came.came, came.published) {
    (Came::Created { renamed: false }, None) => "import.result.link.draft",
    (Came::Created { renamed: true }, None) => "import.result.link.draftRenamed",
    (Came::Created { renamed: false }, Some(_)) => "import.result.link.published",
    (Came::Created { renamed: true }, Some(_)) => "import.result.link.publishedRenamed",
    (Came::Reused, Some(_)) => "import.result.link.reused",
    (Came::Reused, None) => "import.result.link.reusedDraft",
  };
  match came.published {
    Some(version) => ctx.t_args(key, [name, ("version", version.get().to_string())]),
    None => ctx.t_args(key, [name]),
  }
}

/// A connection, MCP server or secret the copies keep naming: a secret by its name in Geist Mono.
fn missing_row(index: usize, missing: &Missing) -> Element {
  let style = match missing.source {
    Entity::Secret(_) => ExtraTypography::PropertyMono.role(),
    Entity::Resource(_) | Entity::Connection(_) | Entity::McpServer(_) => ExtraTypography::Property.role(),
  };
  let text = Text::new(&missing.name)
    .id(format!("pipeline-imported-missing-{index}"))
    .variant(style)
    .color(PaletteColor::TextSecondary)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .nowrap()
    .text_overflow(TextOverflow::Elipsis);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(MISSING_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleDashed, IconSize::Base, PaletteColor::TextMuted))
    .child(text)
    .into()
}
