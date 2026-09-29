//! What an import made (`Z8mGB0`): a row per draft, what was not imported and why, and where to
//! review the drafts.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::ResourceKind;

use super::{BUTTON_HEIGHT, FOOTER_HEIGHT, copy};
use crate::{
  navigation::route::{InstructionsTab, Page},
  screens::project::ProjectNav,
  skills::repository::{CreatedDraft, RepositoryScan},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
  },
};

/// A created draft's row height (`l3hyb`).
const ROW_HEIGHT: f32 = 46.0;

/// The drafts an import created, in the order of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created(pub Vec<CreatedDraft>);

impl lurq::app::component::DevtoolsInspectable for Created {}

pub(super) fn result(ctx: &Ctx, Created(created): &Created, scan: &RepositoryScan, nav: &ProjectNav) -> Element {
  let count = created.len();
  let title = if count == 1 {
    ctx.t("skills.import.result.title.one")
  } else {
    ctx.t_args(
      "skills.import.result.title.other",
      [("count", count.to_string().as_str())],
    )
  };
  let header = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&title)
        .variant(ExtraTypography::DialogTitle.role())
        .id("repository-import-title"),
    )
    .child(Text::new(&ctx.t("skills.import.result.explanation")).variant(ExtraTypography::Explanation.role()));
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Md)
    .with_children(created.iter().map(|draft| created_row(ctx, draft)));
  let left_out: Vec<std::sync::Arc<str>> = scan
    .items
    .iter()
    .filter(|item| !created.iter().any(|draft| draft.source == item.source))
    .map(|item| copy::not_imported(ctx, item))
    .collect();
  let mut column = Column::new().width(Dimension::Pct(100.0)).child(header).child(rows);
  if !left_out.is_empty() {
    column = column.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_horizontal(ExtraSpacing::Rail.role())
        .padding_bottom(ExtraSpacing::Step.role())
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t("skills.import.result.notImported")).variant(ExtraTypography::Overline.role()))
        .child(
          Text::new(&left_out.join(" "))
            .variant(ExtraTypography::Detail.role())
            .id("repository-import-left-out"),
        ),
    );
  }
  column.child(actions(ctx, created, nav)).into()
}

/// A draft (`l3hyb`): its name in mono and what it is.
fn created_row(ctx: &Ctx, draft: &CreatedDraft) -> Element {
  let (glyph, what) = match draft.resource.kind() {
    ResourceKind::Skill if draft.files == 0 => (Icon::Sparkles, ctx.t("skills.import.result.skill")),
    ResourceKind::Skill => (
      Icon::Sparkles,
      ctx.t_args(
        "skills.import.result.skillWithFiles",
        [("files", &*copy::file_count(ctx, draft.files))],
      ),
    ),
    _ => (Icon::BookOpen, ctx.t("skills.import.result.instruction")),
  };
  // The check row (`l3hyb`) as `Z8mGB0` fills it: the name in mono, what it is in the UI font.
  Row::new()
    .id(format!("repository-import-created-{}", draft.resource.slug().as_str()))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Stack.role())
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(glyph, IconSize::Option, PaletteColor::TextPrimary))
    .child(
      Text::new(draft.resource.name().as_str())
        .variant(ExtraTypography::MonoTitle.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .child(Text::new(&what).variant(TypographyStyle::Caption).nowrap())
    .into()
}

/// Open Instructions for imported instructions, Review skills for skills (`k5PGor`).
fn actions(ctx: &Ctx, created: &[CreatedDraft], nav: &ProjectNav) -> Element {
  let first = |kind: ResourceKind| created.iter().find(|draft| draft.resource.kind() == kind);
  let instruction = first(ResourceKind::Instruction).map(|draft| draft.resource.id());
  let skill = first(ResourceKind::Skill).map(|draft| draft.resource.id());
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Spacer::new().flex(1.0));
  if let Some(id) = instruction {
    let nav = nav.clone();
    let button = ActionButton::new(ctx.t("skills.import.result.openInstructions"))
      .icon(Icon::BookOpen)
      .id("repository-import-open-instructions")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(move || nav.leave_repository_import(Page::Instructions(InstructionsTab::ProjectRules, Some(id))));
    row = row.child(if skill.is_some() { button } else { button.primary() }.render(ctx));
  }
  if let Some(id) = skill {
    let nav = nav.clone();
    row = row.child(
      ActionButton::new(ctx.t("skills.import.result.reviewSkills"))
        .primary()
        .icon(Icon::Sparkles)
        .id("repository-import-review-skills")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || nav.leave_repository_import(Page::Skills(Some(id))))
        .render(ctx),
    );
  }
  row.into()
}
