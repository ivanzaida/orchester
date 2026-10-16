//! XNgaG: returned import metadata, readable origins and explicit navigation availability.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::ResourceKind;
use orchester_workspace::skills::repository::CreatedDraft;

use super::{PageProps, repository::item_summary, state::Panel, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon, marker},
    inspector::note,
  },
};
/// Z20cmw metadata column; flexible title/source text takes all remaining row width.
const META_WIDTH: f32 = 220.0;

pub fn result(ctx: &mut Ctx, p: &PageProps, created: &[CreatedDraft], total: usize) -> Element {
  let instructions = created
    .iter()
    .filter(|d| d.resource.kind() == ResourceKind::Instruction)
    .count();
  let skills = created
    .iter()
    .filter(|d| d.resource.kind() == ResourceKind::Skill)
    .count();
  let totals = words::types(ctx, instructions, skills);
  let mut rows = Column::new().width(Dimension::Pct(100.0));
  for (i, draft) in created.iter().enumerate() {
    rows = rows.child(created_row(ctx, p, draft, i));
  }
  let mut content = Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(820.0))
    .spacing(crate::theme::skills::result_spacing())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&ctx.t_args("skills.created", [("types", totals.to_uppercase())]))
            .variant(crate::theme::skills::import_caption()),
        )
        .child(rows),
    );
  content = content.child(omitted(ctx, p, created)).child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(note(
        "skill-import-result-note",
        Icon::Info,
        &ctx.t("skills.resultNote"),
      ))
      .child(note(
        "skill-import-repository-note",
        Icon::FolderGit2,
        &ctx.t("skills.resultRepository"),
      )),
  );
  Column::new()
    .id("skills-import-result")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header(ctx, p, created.len()))
    .child(
      ScrollVertical::new(
        Column::new()
          .id("skill-result-body")
          .width(Dimension::Pct(100.0))
          .padding(Padding::symmetric(
            SpacingSize::Xl,
            crate::theme::skills::result_header_bottom(),
          ))
          .child(content),
      )
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0)),
    )
    .child(footer(ctx, p, &totals, (created.len(), total)))
    .into()
}
fn created_row(ctx: &Ctx, p: &PageProps, draft: &CreatedDraft, i: usize) -> Element {
  let skill = draft.resource.kind() == ResourceKind::Skill;
  let label = draft_label(ctx, draft);
  let panel = p.view.panel.clone();
  let id = draft.resource.id();
  let open = ActionButton::new(
    format!("skill-import-open-{i}"),
    ButtonKind::Ghost,
    ctx.t("skills.openDraft"),
  )
  .icon(Icon::ArrowUpRight);
  let open = if skill {
    open.on_click(move || panel.set(Panel::Edit(id)))
  } else {
    open.unavailable(ctx.t("skills.instructionEditorUnavailable"))
  };
  Row::new()
    .id(format!("skill-created-row-{i}"))
    .align_items(lurq::layout::Alignment::Center)
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(54.0))
    .spacing(crate::theme::skills::cell_spacing())
    .padding(Padding::symmetric(
      crate::theme::skills::cell_spacing(),
      ExtraSpacing::Loose.role(),
    ))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(marker(if skill { Icon::GraduationCap } else { Icon::BookOpen }, 28.0))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(draft.resource.name().as_str())
            .id(format!("skill-created-name-{i}"))
            .width(Dimension::Pct(100.0))
            .variant(crate::theme::skills::name()),
        )
        .child(
          Text::new(&ctx.t_args("skills.resultFrom", [("source", draft.source.as_str())]))
            .id(format!("skill-created-source-{i}"))
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Caption),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Px(META_WIDTH))
        .spacing(ExtraSpacing::Snug.role())
        .child(icon(Icon::FilePenLine, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&label)
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .variant(ExtraTypography::Note.role()),
        ),
    )
    .child(open)
    .into()
}
fn header(ctx: &mut Ctx, p: &PageProps, count: usize) -> Column {
  let back = p.view.panel.clone();
  let crumbs = breadcrumb(
    ctx,
    "skill-result-crumb",
    CrumbSeparator::Chevron,
    vec![
      Crumb::link(ctx.t("skills.title"), move || back.set(Panel::LIBRARY)),
      Crumb::here(ctx.t("skills.import")),
    ],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Xl)
        .right(SpacingSize::Xl)
        .top(ExtraSpacing::Inset.role())
        .bottom(crate::theme::skills::result_header_bottom()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(crumbs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::CircleCheck, IconSize::Xl, PaletteColor::Success))
        .child(
          Text::new(&ctx.t_args("skills.importDone", [("count", count.to_string())])).variant(TypographyStyle::Heading),
        ),
    )
    .child(
      Text::new(&ctx.t("skills.importResultDetail"))
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Description),
    )
}
fn omitted(ctx: &Ctx, p: &PageProps, created: &[CreatedDraft]) -> Column {
  let mut omitted = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t("skills.notImported")).variant(ExtraTypography::ColumnHeading.role()));
  for item in p
    .data
    .repository
    .items
    .iter()
    .filter(|i| !created.iter().any(|d| d.source == i.source))
  {
    omitted = omitted.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(SpacingSize::Sm)
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::Minus, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&item.source)
            .width(Dimension::Px(230.0))
            .variant(TypographyStyle::Mono),
        )
        .child(
          Text::new(&if item.outcome.is_err() {
            item_summary(ctx, item)
          } else {
            ctx.t("skills.notSelected")
          })
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .variant(ExtraTypography::Note.role()),
        ),
    );
  }
  omitted
}
fn footer(ctx: &Ctx, p: &PageProps, totals: &str, (imported, total): (usize, usize)) -> Row {
  let panel = p.view.panel.clone();
  Row::new()
    .id("skill-result-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(56.0))
    .align_items(lurq::layout::Alignment::Center)
    .padding(Padding::horizontal(SpacingSize::Xl))
    .spacing(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t_args(
        "skills.importedTotals",
        [
          ("types", totals.to_owned()),
          ("imported", imported.to_string()),
          ("total", total.to_string()),
        ],
      ))
      .variant(ExtraTypography::Note.role()),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(
        "skill-import-instructions",
        ButtonKind::Secondary,
        ctx.t("skills.openInstructions"),
      )
      .unavailable(ctx.t("skills.instructionEditorUnavailable")),
    )
    .child(
      ActionButton::new("skills-back", ButtonKind::Primary, ctx.t("skills.reviewSkills"))
        .on_click(move || panel.set(Panel::LIBRARY)),
    )
}
fn draft_label(ctx: &Ctx, draft: &CreatedDraft) -> std::sync::Arc<str> {
  let skill = draft.resource.kind() == ResourceKind::Skill;
  ctx.t_args(
    if skill {
      if draft.files == 0 {
        "skills.resultSkill"
      } else {
        "skills.resultSkillFiles"
      }
    } else {
      "skills.resultInstruction"
    },
    [
      ("version", draft.version.get().to_string()),
      ("files", words::files(ctx, draft.files).to_string()),
    ],
  )
}
