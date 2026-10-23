//! What Create drafts lists (`What happens` `zuLiO`, `Policy and access` `f8PUy`): the drafts the import creates,
//! this project's own resources it uses instead, what it leaves unresolved, which callers the copied tools keep, and
//! that importing authorizes nothing.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::ToolKind;
use orchester_workspace::import::{
  drafts::{Access, Created, Drafts, Missing, Reused},
  review::Subject,
};

use super::{frame, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ImportSpacing},
  ui::{
    icon::{Icon, icon},
    inspector::note,
    unavailable::reason_line,
  },
};

/// A row's glyph box, its kind column, the access rows' height and their tool column (`U0hvm`, `M816v7`,
/// `S9pMRY`, `YuYIw`).
const GLYPH: f32 = 28.0;
const KIND_WIDTH: f32 = 220.0;
const ACCESS_ROW_HEIGHT: f32 = 34.0;
const TOOL_WIDTH: f32 = 130.0;
/// The policy and access column (`f8PUy`).
pub(super) const POLICY_WIDTH: f32 = 440.0;

/// The drafts created, the resources reused and what is left unresolved, each under its eyebrow.
pub(super) fn what_happens(ctx: &Ctx, drafts: &Drafts, (source, target): (&str, &str)) -> Column {
  let created: Vec<Element> = drafts
    .created
    .iter()
    .enumerate()
    .map(|(index, created)| created_row(ctx, index, created, source))
    .collect();
  let mut sections = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ImportSpacing::Sections.role())
    .child(section(
      "import-created",
      &words::counted(
        ctx,
        drafts.created.len(),
        "import.ready.creates.one",
        "import.ready.creates.other",
      ),
      created,
    ));
  if !drafts.reused.is_empty() {
    let rows = drafts
      .reused
      .iter()
      .enumerate()
      .map(|(index, reused)| reused_row(ctx, index, reused, (source, target)))
      .collect();
    sections = sections.child(section(
      "import-reused",
      &words::counted(
        ctx,
        drafts.reused.len(),
        "import.ready.reuses.one",
        "import.ready.reuses.other",
      ),
      rows,
    ));
  }
  if !drafts.missing.is_empty() {
    let rows = drafts
      .missing
      .iter()
      .enumerate()
      .map(|(index, missing)| missing_row(ctx, index, missing, target))
      .collect();
    let eyebrow = ctx.t_args("import.ready.unresolved", [("count", drafts.missing.len().to_string())]);
    sections = sections.child(section("import-missing", &eyebrow, rows));
  }
  sections
}

/// An eyebrow over its rows, which a line starts (`Section · CREATES` `Um8nN`).
fn section(id: &str, eyebrow: &Arc<str>, rows: Vec<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(frame::eyebrow(&format!("{id}-eyebrow"), eyebrow))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .with_children(rows),
    )
}

/// A glyph in its box (`Icon box` `U0hvm`).
fn glyph_box(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH))
    .height(Dimension::Px(GLYPH))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary))
}

/// A row of the list (`Created draft row` `Z20cmw`): a glyph, a name over where it comes from, and what it is.
fn listed_row(id: &str, glyph: Icon, (name, from): (&str, &str), (mark, kind): (Icon, &str)) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(glyph_box(glyph))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(name)
            .id(format!("{id}-name"))
            .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(from)
            .id(format!("{id}-from"))
            .width(Dimension::Pct(100.0))
            .variant(ExtraTypography::Note.role()),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Px(KIND_WIDTH))
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(mark, IconSize::Base, PaletteColor::TextSecondary))
        .child(
          Text::new(kind)
            .id(format!("{id}-kind"))
            .variant(TypographyStyle::Description)
            .nowrap(),
        ),
    )
    .into()
}

fn created_row(ctx: &Ctx, index: usize, created: &Created, source: &str) -> Element {
  let version = words::version(ctx, Some(created.pick));
  let from = ctx.t_args(
    if created.renamed {
      "import.ready.fromRenamed"
    } else {
      "import.ready.from"
    },
    [
      ("project", source),
      ("name", created.source_name.as_str()),
      ("version", &version),
    ],
  );
  let kind = ctx.t(draft_kind(created.subject));
  listed_row(
    &format!("import-created-{index}"),
    words::glyph(created.subject),
    (&created.name, &from),
    (Icon::FilePenLine, &kind),
  )
}

const fn draft_kind(subject: Subject) -> &'static str {
  match subject {
    Subject::Pipeline => "import.ready.draftKind.pipeline",
    Subject::AgentProfile => "import.ready.draftKind.agentProfile",
    Subject::Instruction => "import.ready.draftKind.instruction",
    Subject::Tool(ToolKind::Http) => "import.ready.draftKind.httpTool",
    Subject::Tool(ToolKind::Database) => "import.ready.draftKind.databaseTool",
    Subject::Tool(ToolKind::Command) | Subject::Connection(_) | Subject::McpServer { .. } | Subject::Secret(_) => {
      "import.ready.draftKind.commandTool"
    }
  }
}

fn reused_row(ctx: &Ctx, index: usize, reused: &Reused, (source, target): (&str, &str)) -> Element {
  let from = match reused.source_pick {
    Some(pick) => ctx.t_args(
      "import.ready.insteadOf",
      [
        ("name", Arc::from(reused.source_name.as_str())),
        ("version", words::version(ctx, Some(pick))),
        ("project", Arc::from(source)),
      ],
    ),
    None => ctx.t_args(
      "import.ready.insteadOfEntity",
      [("name", reused.source_name.as_str()), ("project", source)],
    ),
  };
  let kind = match (reused.subject.resource_kind(), reused.published) {
    (Some(_), Some(version)) => ctx.t_args(
      "import.ready.inTarget.published",
      [("version", version.get().to_string()), ("project", target.to_owned())],
    ),
    (Some(_), None) => ctx.t_args("import.ready.inTarget.draft", [("project", target)]),
    (None, _) => ctx.t_args("import.ready.inTarget.own", [("project", target)]),
  };
  listed_row(
    &format!("import-reused-{index}"),
    words::glyph(reused.subject),
    (&reused.name, &from),
    (Icon::Link, &kind),
  )
}

/// What the import leaves unresolved, and what that means for the copies (`Not imported row` `fuWWh`).
fn missing_row(ctx: &Ctx, index: usize, missing: &Missing, target: &str) -> Element {
  let users = missing.users.join(", ");
  let reason = match missing.subject {
    Subject::Secret(_) => ctx.t_args(
      if missing.users.len() == 1 {
        "import.ready.missing.secretOne"
      } else {
        "import.ready.missing.secretOther"
      },
      [("users", users.as_str()), ("project", target)],
    ),
    Subject::McpServer { .. } => ctx.t_args("import.ready.missing.mcpServer", [("project", target)]),
    _ => ctx.t_args("import.ready.missing.connection", [("project", target)]),
  };
  let name = Text::new(&missing.name)
    .id(format!("import-missing-{index}-name"))
    .nowrap();
  let name = if matches!(missing.subject, Subject::Secret(_)) {
    name.variant(TypographyStyle::Mono).color(PaletteColor::TextSecondary)
  } else {
    name.variant(TypographyStyle::Description)
  };
  Row::new()
    .id(format!("import-missing-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ImportSpacing::MissingRow.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::CircleDashed, IconSize::Base, PaletteColor::TextMuted))
    .child(name)
    .child(
      Text::new(&reason)
        .id(format!("import-missing-{index}-reason"))
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// The target access policy, what is authorized in this project, and where each copy comes from.
pub(super) fn policy_and_access(ctx: &Ctx, access: &[Access], (source, target): (&str, &str)) -> Column {
  let mut policy = card("import-policy", Icon::ShieldCheck, &ctx.t("import.policy.title"), None)
    .child(detail(&ctx.t("import.policy.detail")));
  for (index, access) in access.iter().enumerate() {
    policy = policy.child(rules(ctx, index, access));
  }
  policy = policy.child(reason_line("import-policy-reason", &ctx.t("import.policy.notApplied")));
  let authorized = card(
    "import-authorized",
    Icon::KeyRound,
    &ctx.t_args("import.authorized.title", [("project", target)]),
    Some(&ctx.t("import.authorized.meta")),
  )
  .child(detail(&ctx.t_args("import.authorized.detail", [("project", target)])));
  Column::new()
    .width(Dimension::Px(POLICY_WIDTH))
    .spacing(SpacingSize::Md)
    .child(policy)
    .child(authorized)
    .child(note(
      "import-provenance",
      Icon::History,
      &ctx.t_args("import.provenance", [("project", source)]),
    ))
}

/// A card with its glyph, title and what it says at the end of its head (`Target access policy` `ollMf`).
fn card(id: &str, glyph: Icon, title: &str, meta: Option<&str>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Loose.role())
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(ExtraTypography::FormSection.role()),
        )
        .child(Spacer::new().flex(1.0))
        .with_children(meta.map(|meta| {
          Text::new(meta)
            .id(format!("{id}-meta"))
            .variant(TypographyStyle::Caption)
        })),
    )
}

fn detail(text: &str) -> Text {
  Text::new(text)
    .width(Dimension::Pct(100.0))
    .variant(TypographyStyle::Description)
}

/// The steps of a copied pipeline that may call its copied tools, then every other caller, denied.
fn rules(ctx: &Ctx, index: usize, access: &Access) -> Column {
  let id = format!("import-rules-{index}");
  let mut rows = Column::new()
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
  for (step, allowed) in access.allowed.iter().enumerate() {
    let scope = ctx.t_args(
      "import.policy.step",
      [
        ("number", format!("{:02}", allowed.step)),
        ("name", allowed.step_name.clone()),
      ],
    );
    rows = rows.child(rule_row(
      format!("{id}-{step}"),
      (words::glyph(Subject::Tool(allowed.tool_kind)), &allowed.tool, &scope),
      (
        Icon::CircleCheck,
        PaletteColor::Success,
        &ctx.t("import.policy.allowed"),
      ),
    ));
  }
  rows = rows.child(rule_row(
    format!("{id}-others"),
    (
      Icon::Ban,
      &ctx.t("import.policy.others"),
      &ctx.t("import.policy.othersScope"),
    ),
    (
      Icon::CircleMinus,
      PaletteColor::TextMuted,
      &ctx.t("import.policy.denied"),
    ),
  ));
  let eyebrow = ctx.t_args("import.policy.steps", [("pipeline", access.pipeline.to_uppercase())]);
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(frame::eyebrow(&format!("{id}-eyebrow"), &eyebrow))
    .child(rows)
}

/// One caller and whether it may call (`Access policy row` `S9pMRY`).
fn rule_row(
  id: String,
  (glyph, tool, scope): (Icon, &str, &str),
  (mark, tint, access): (Icon, PaletteColor, &str),
) -> Row {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACCESS_ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(tool)
        .width(Dimension::Px(TOOL_WIDTH))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(scope)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(mark, IconSize::Base, tint))
        .child(Text::new(access).variant(TypographyStyle::Description).nowrap()),
    )
}
