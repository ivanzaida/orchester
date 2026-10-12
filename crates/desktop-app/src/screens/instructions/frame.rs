//! Header p6Bsv, scope tabs p7ztk and empty state B5A4Ux.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
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
  let import = selection.clone();
  let new = selection.clone();
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
      .on_click(move || import.open(View::Import)),
    )
    .child(
      ActionButton::new("instructions-new", ButtonKind::Primary, ctx.t("instructions.new"))
        .icon(Icon::Plus)
        .on_click(move || new.open(View::New(tab))),
    );
  let tabs = Tab::ALL
    .into_iter()
    .enumerate()
    .map(|(index, choice)| {
      let selection = selection.clone();
      let count = data
        .entries
        .iter()
        .filter(|entry| Tab::of(entry.body.scope()) == choice)
        .count();
      PageTab::new(
        format!("instruction-tab-{index}"),
        counted(ctx, &ctx.t(choice.key()), count),
        tab == choice,
        move || selection.open(View::Rule(choice, None)),
      )
    })
    .collect::<Vec<_>>();
  let tabs = page_tabs(ctx, "instructions-tabs", tabs);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Tabs.role())
        .right(ExtraSpacing::Tabs.role())
        .top(instructions::head_top()),
    )
    .spacing(SpacingSize::Md)
    .child(title)
    .child(tabs)
    .into()
}

pub fn empty(ctx: &mut Ctx, project: &ShownProject, data: &ProjectInstructions, selection: &Selection) -> Element {
  ctx.mount_keyed::<super::empty::Empty>(
    "instructions-empty",
    super::empty::Props {
      project: project.clone(),
      selection: selection.clone(),
      all_empty: data.entries.is_empty(),
    },
  )
}
