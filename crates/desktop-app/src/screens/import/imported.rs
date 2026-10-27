//! What a copied pipeline shows of the import that created it (`06b · imported pipeline` `ky5kJ`): the strip under
//! the page's header while it is not published (`Import result` `H2g3Hs`), and the step column's Imported, Imported
//! with it and Missing groups (`W1I5r6`, `pevvn`, `JqeBT`). What it says comes from
//! `orchester_workspace::import::imported`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
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
  theme::{ExtraSpacing, ExtraTypography, GroupText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    plan_window,
    pressable::Press,
    row_group::{GroupRow, RowGroup, note},
  },
};

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
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::CircleCheck, IconSize::Md, PaletteColor::Success))
    .child(
      Text::new(&ctx.t_args("import.result.title", [("project", imported.project.as_str())]))
        .id("pipeline-import-title")
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t_args("import.result.summary", [("totals", &*totals(ctx, imported))]))
        .id("pipeline-import-summary")
        .variant(GroupText::Hint.role())
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

/// The step column's groups on the import, in `target`, this project, by its name: Imported, then what came with
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
    RowGroup::new("pipeline-imported", ctx.t("import.result.imported"))
      .count(when)
      .row(GroupRow::new("pipeline-imported-from", ctx.t("import.result.from")).value(&imported.project))
      .row(GroupRow::new("pipeline-imported-source", ctx.t("import.result.sourceVersion")).value(&source))
      .after(note(
        "pipeline-imported-note",
        &ctx.t_args("import.result.independent", [("project", imported.project.as_str())]),
      ))
      .into(),
  ];
  if !imported.with_it.is_empty() {
    let links: Vec<GroupRow> = imported.with_it.iter().map(|came| link(ctx, project, came)).collect();
    all.push(
      RowGroup::new("pipeline-imported-with", ctx.t("import.result.withIt"))
        .count(imported.with_it.len().to_string())
        .rows(links)
        .into(),
    );
  }
  if !imported.missing.is_empty() {
    all.push(
      RowGroup::new(
        "pipeline-imported-missing",
        ctx.t_args("import.result.missing", [("project", target)]),
      )
      .count(imported.missing.len().to_string())
      .rows(
        imported
          .missing
          .iter()
          .enumerate()
          .map(|(index, missing)| missing_row(index, missing)),
      )
      .after(note(
        "pipeline-imported-missing-note",
        &ctx.t_args("import.result.missingNote", [("project", target)]),
      ))
      .into(),
    );
  }
  all
}

/// A resource that came along with the copy, which opens its page.
fn link(ctx: &mut Ctx, project: ProjectId, came: &WithIt) -> GroupRow {
  let page = page_of(came.id, came.content.kind(), Some(&came.content));
  let open = Navigation::of(ctx).to(Route::Project(project, page));
  GroupRow::new(format!("pipeline-imported-link-{}", came.id), link_label(ctx, came))
    .marked()
    .leading(icon(glyph(&came.content), IconSize::Md, PaletteColor::TextMuted))
    .end(icon(Icon::ArrowUpRight, IconSize::Base, PaletteColor::TextMuted))
    .press(Press::Open(open))
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

/// A connection, MCP server or secret the copies keep naming, with the glyph of what it is: a secret by its name
/// in Geist Mono.
fn missing_row(index: usize, missing: &Missing) -> GroupRow {
  let glyph = match missing.source {
    Entity::Secret(_) => Icon::KeyRound,
    Entity::Connection(_) => Icon::Plug,
    Entity::McpServer(_) => Icon::Server,
    Entity::Resource(_) => Icon::CircleDashed,
  };
  let row = GroupRow::new(format!("pipeline-imported-missing-{index}"), missing.name.clone())
    .marked()
    .leading(icon(glyph, IconSize::Md, PaletteColor::TextMuted));
  match missing.source {
    Entity::Secret(_) => row.mono(),
    Entity::Resource(_) | Entity::Connection(_) | Entity::McpServer(_) => row,
  }
}
