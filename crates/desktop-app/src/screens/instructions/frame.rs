//! Header p6Bsv, scope tabs p7ztk and empty state B5A4Ux.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::instructions::data::ProjectInstructions;

use super::state::{Selection, Tab, View};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    tabs::{PageTab, counted, page_tabs},
  },
};

pub fn header(
  ctx: &mut Ctx,
  project: &ShownProject,
  data: &ProjectInstructions,
  tab: Tab,
  selection: &Selection,
) -> Element {
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t("instructions.title")).variant(TypographyStyle::Heading))
        .child(
          Text::new(&ctx.t_args(
            "instructions.description",
            [("project", project.open.project.name().as_str())],
          ))
          .variant(TypographyStyle::Description),
        ),
    )
    .child(
      ActionButton::new(
        "instructions-import",
        ButtonKind::Secondary,
        ctx.t("instructions.import"),
      )
      .icon(Icon::FolderOpen)
      .opens(selection.to(View::Import)),
    )
    .child(
      ActionButton::new("instructions-new", ButtonKind::Primary, ctx.t("instructions.new"))
        .icon(Icon::Plus)
        .opens(selection.to(View::new_rule(tab))),
    );
  let tabs = Tab::ALL
    .into_iter()
    .enumerate()
    .map(|(index, choice)| {
      let count = data
        .entries
        .iter()
        .filter(|entry| Tab::of(entry.body.scope()) == choice)
        .count();
      PageTab::to(
        format!("instruction-tab-{index}"),
        counted(ctx, &ctx.t(choice.key()), count),
        tab == choice,
        selection.to(View::rule(choice, None)),
      )
    })
    .collect::<Vec<_>>();
  let tabs = page_tabs(ctx, "instructions-tabs", tabs);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      title.padding(
        Padding::new()
          .left(ExtraSpacing::Tabs.role())
          .right(ExtraSpacing::Tabs.role())
          .top(instructions::head_top()),
      ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(tabs),
    )
    .into()
}

pub fn empty(
  ctx: &mut Ctx,
  project: &ShownProject,
  (data, tab): (&ProjectInstructions, Tab),
  selection: &Selection,
) -> Element {
  ctx.mount_keyed::<super::empty::Empty>(
    "instructions-empty",
    super::empty::Props {
      project: project.clone(),
      selection: selection.clone(),
      all_empty: data.entries.is_empty(),
      tab,
    },
  )
}
