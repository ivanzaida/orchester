//! Resolve dependencies (`xp4dr` blocked, `jsMUX` ready; its rows' states in `HpzO7`): the selected resources and
//! everything their copies refer to, each with how it is resolved in this project, then Continue to Create drafts
//! once nothing blocks.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::import::{Entity, Item, Plan, Role};
use orchester_workspace::import::{data::Catalogs, review};

use super::{
  State, Step, frame,
  mapping_row::{self, MappingRowProps, Offered, SOURCE_WIDTH, TARGET_WIDTH},
  select::body,
  words,
};
use crate::{
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::note,
  },
};

/// The table's heading row and a group's (`Mapping header` `WNkw7`, `Group · SELECTED · 1` `rEYGJ`).
const HEADING_HEIGHT: f32 = 32.0;
const GROUP_HEIGHT: f32 = 28.0;

/// The step's body and its footer.
pub(super) fn step(ctx: &mut Ctx, state: &State, catalogs: &Catalogs) -> (Element, Element) {
  let plan = state.plan(catalogs);
  let target = catalogs.target.project().name().as_str();
  let (selected, dependencies): (Vec<&Item>, Vec<&Item>) =
    plan.items().iter().partition(|item| item.role() == Role::Selected);
  let mut table = Column::new()
    .id("import-mappings")
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Lg)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .clip()
    .child(headings(ctx, target));
  for (id, key, items) in [
    ("import-group-selected", "import.groups.selected", &selected),
    ("import-group-dependencies", "import.groups.dependencies", &dependencies),
  ] {
    if items.is_empty() {
      continue;
    }
    table = table.child(group(id, &ctx.t_args(key, [("count", items.len().to_string())])));
    for item in items.iter() {
      table = table.child(row(ctx, state, catalogs, item));
    }
  }
  let body = body(crate::theme::ExtraSpacing::Inset.role())
    .child(context(ctx, catalogs, &plan))
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .padding_bottom(SpacingSize::Lg)
          .child(table)
          .child(note(
            "import-access-note",
            Icon::ShieldCheck,
            &ctx.t_args("import.accessNote", [("project", target)]),
          )),
      )
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0)),
    );
  (body.into(), footer(ctx, state, catalogs, &plan))
}

/// "Personal website → Kontur · Preview release v3" (`Context` `tKBxS`).
fn context(ctx: &Ctx, catalogs: &Catalogs, plan: &Plan) -> Row {
  let projects = (
    catalogs.source.project().name().as_str(),
    catalogs.target.project().name().as_str(),
  );
  Row::new()
    .id("import-context")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::FolderOpen, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words::context(ctx, projects, &review::selection(catalogs, plan)))
        .id("import-context-text")
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
}

/// SOURCE / VERSION, TARGET IN <PROJECT> and RESOLUTION (`WNkw7`).
fn headings(ctx: &Ctx, target: &str) -> Row {
  let heading = |text: &str| Text::new(text).variant(ExtraTypography::ColumnHeading.role()).nowrap();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Px(SOURCE_WIDTH))
        .child(heading(&ctx.t("import.columns.source"))),
    )
    .child(Row::new().width(Dimension::Px(TARGET_WIDTH)).child(heading(
      &ctx.t_args("import.columns.target", [("project", target.to_uppercase())]),
    )))
    .child(Row::new().flex(1.0).child(heading(&ctx.t("import.columns.resolution"))))
}

/// A group's eyebrow on its band (`rEYGJ`).
fn group(id: &str, text: &std::sync::Arc<str>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(frame::eyebrow(id, text))
}

fn row(ctx: &mut Ctx, state: &State, catalogs: &Catalogs, item: &Item) -> Element {
  let entity = item.source();
  let target = catalogs.target.project().name().as_str().to_owned();
  let subject = review::subject(&catalogs.source, entity, item.pick()).unwrap_or(review::Subject::Pipeline);
  let name = review::name(&catalogs.source, entity);
  let (look, status, status_detail) = words::status(ctx, &target, &review::status(catalogs, item));
  let offered = item
    .options()
    .iter()
    .map(|choice| {
      let (offer, detail) = review::offer(catalogs, item, *choice);
      let (label, detail) = words::offer(ctx, &target, &offer, detail.as_ref());
      Offered {
        choice: *choice,
        label,
        detail,
      }
    })
    .collect();
  let planned = review::planned(item)
    .iter()
    .map(|planned| {
      let (label, reason) = words::planned(ctx, &target, *planned);
      (*planned, label, reason)
    })
    .collect();
  let typed = state.choices.get().0.name(entity).map(str::to_owned);
  let props = MappingRowProps {
    entity,
    rename: typed.unwrap_or_else(|| name.clone()),
    name: name.into(),
    mono: matches!(entity, Entity::Secret(_)),
    glyph: words::glyph(subject),
    detail: words::detail(ctx, subject, item.pick()),
    offered,
    planned,
    choice: item.choice(),
    look,
    status,
    status_detail,
    choices: state.choices.clone(),
  };
  mapping_row::mount(ctx, props)
}

/// The totals, what keeps the import from going on or what it creates, Back and Continue (`hnvcD`, `NRL4F`).
fn footer(ctx: &mut Ctx, state: &State, catalogs: &Catalogs, plan: &Plan) -> Element {
  let (ready, said) = words::readiness(ctx, &review::readiness(catalogs, plan));
  let step = state.step.clone();
  let back = ActionButton::new("import-previous", ButtonKind::Secondary, ctx.t("import.previous"))
    .icon(Icon::ArrowLeft)
    .on_click(move || step.set(Step::Select));
  let step = state.step.clone();
  let next = ActionButton::new("import-continue", ButtonKind::Primary, ctx.t("import.continue")).icon(Icon::ArrowRight);
  let (line, next) = if ready {
    (frame::ready_line(&said), next.on_click(move || step.set(Step::Create)))
  } else {
    (frame::blocked(&said), next.unavailable(said.clone()))
  };
  frame::footer(&words::totals(ctx, plan.counts()), vec![line, back.into(), next.into()])
}
