use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::LocalFile;
use orchester_workspace::tasks::local_integration::{IntegrationView, Preview};

use super::{LocalPage, Mode};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  code_box::CodeBox,
  toggle::{Toggle, ToggleKind},
};

pub(super) fn content(ctx: &mut Ctx, page: &LocalPage, view: &IntegrationView, busy: bool) -> Element {
  let mut column = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Md);
  if let Mode::Commit { preview: shown, .. } = page.mode.get() {
    column = column.child(diff_preview(
      ctx,
      "local-selected-diff",
      "local_integration.selected",
      &shown,
    ));
    column = column.child(Text::new(&ctx.t("local_integration.excluded")).variant(TypographyStyle::Description));
  } else if view.operation.as_ref().is_some_and(|op| op.checked().is_some()) {
    column = column.child(preview(
      ctx,
      "local-checked-diff",
      "local_integration.checked",
      &view.diff,
    ));
  } else {
    column = column.child(Text::new(&ctx.t("local_integration.files")).variant(TypographyStyle::Title));
    column = column.child(file_list(
      ctx,
      page,
      view,
      busy || page.mode.get() != Mode::Review || view.external.is_some(),
    ));
    column = column.child(diff_preview(
      ctx,
      "local-file-diff",
      "local_integration.review",
      &view.diff,
    ));
  }
  column.into()
}

fn file_row(
  ctx: &Ctx,
  selected_paths: &Signal<super::Selection>,
  shown_file: &Signal<super::ShownFile>,
  file: &LocalFile,
  busy: bool,
  group: &str,
) -> Element {
  let selected = selected_paths.get().contains(&file.path);
  let select = selected_paths.clone();
  let path = file.path.clone();
  let mut choice = Toggle::new(
    format!("local-select-{group}-{path}"),
    ToggleKind::Checkbox,
    selected,
    path.to_string(),
  );
  if !busy {
    choice = choice.on_change(move |on| {
      select.update(|paths| {
        paths.retain(|value| value != &path);
        if on {
          paths.push(path.clone());
        }
      })
    });
  }
  let show = shown_file.clone();
  let path = file.path.clone();
  let inspect = ActionButton::new(
    format!("local-file-{group}-{path}"),
    ButtonKind::Ghost,
    file.path.to_string(),
  )
  .on_click(move || show.set(super::ShownFile(Some(path.clone()))));
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(56.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(choice.hide_label())
        .child(inspect),
    )
    .child(Text::new(&ctx.t(group)).variant(TypographyStyle::Caption))
    .into()
}

pub(super) fn preview(ctx: &mut Ctx, id: &str, label: &str, shown: &Preview) -> Element {
  match shown {
    Preview::Text(text) => CodeBox::new(id, ctx.t(label), text.as_str()).render(ctx),
    other => {
      let key = match other {
        Preview::Binary => "local_integration.binary",
        Preview::Sensitive => "local_integration.sensitive",
        _ => "local_integration.absent",
      };
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Label))
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Description))
        .into()
    }
  }
}
#[derive(Clone, PartialEq)]
enum FileItem {
  Group(&'static str),
  File(LocalFile, &'static str),
}
#[derive(Clone)]
struct FileProps {
  item: FileItem,
  selected: Signal<super::Selection>,
  shown: Signal<super::ShownFile>,
  busy: bool,
}
impl PartialEq for FileProps {
  fn eq(&self, other: &Self) -> bool {
    match (&self.item, &other.item) {
      (FileItem::Group(a), FileItem::Group(b)) => a == b && self.busy == other.busy,
      (FileItem::File(a, ga), FileItem::File(b, gb)) => a == b && ga == gb && self.busy == other.busy,
      _ => false,
    }
  }
}
impl DevtoolsInspectable for FileProps {}
struct FileRow;
impl Component for FileRow {
  type Props = FileProps;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FileProps>();
    match &props.item {
      FileItem::Group(label) => Column::new()
        .height(Dimension::Px(40.0))
        .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Label))
        .into(),
      FileItem::File(file, group) => file_row(ctx, &props.selected, &props.shown, file, props.busy, group),
    }
  }
}
fn file_list(ctx: &mut Ctx, page: &LocalPage, view: &IntegrationView, busy: bool) -> Element {
  let mut rows = Vec::new();
  for group in [
    "local_integration.staged",
    "local_integration.unstaged",
    "local_integration.untracked",
  ] {
    let files = view
      .checkout
      .files
      .iter()
      .filter(|file| match group {
        "local_integration.staged" => file.staged != file.head,
        "local_integration.unstaged" => file.unstaged,
        _ => file.head.is_none() && file.staged.is_none(),
      })
      .cloned()
      .collect::<Vec<_>>();
    if !files.is_empty() {
      rows.push(FileItem::Group(group));
      rows.extend(files.into_iter().map(|file| FileItem::File(file, group)));
    }
  }
  let (selected, shown) = (page.selected.clone(), page.shown_file.clone());
  let height = (rows.len() as f32 * 56.0).clamp(56.0, 320.0);
  VirtualizedList::new_shared(ctx, Arc::new(rows))
    .list_key("local-files")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .mount_keyed::<FileRow, _, _, _>(
      |item| match item {
        FileItem::Group(label) => (*label).to_owned(),
        FileItem::File(file, group) => format!("{group}:{}", file.path),
      },
      move |item| FileProps {
        item: item.clone(),
        selected: selected.clone(),
        shown: shown.clone(),
        busy,
      },
    )
}
fn diff_preview(ctx: &mut Ctx, id: &str, label: &str, shown: &Preview) -> Element {
  let Preview::Text(text) = shown else {
    return preview(ctx, id, label, shown);
  };
  let rows = Arc::new(
    text
      .lines()
      .enumerate()
      .map(|(index, text)| DiffLine {
        index,
        text: text.to_owned(),
      })
      .collect::<Vec<_>>(),
  );
  let height = (rows.len() as f32 * 20.0).clamp(40.0, 280.0);
  let list = VirtualizedList::new_shared(ctx, rows)
    .list_key(id)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .mount_keyed::<DiffRow, _, _, _>(|row| row.index, Clone::clone);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Label))
    .child(list)
    .into()
}
#[derive(Clone, PartialEq)]
struct DiffLine {
  index: usize,
  text: String,
}
impl DevtoolsInspectable for DiffLine {}
struct DiffRow;
impl Component for DiffRow {
  type Props = DiffLine;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    use crate::theme::ExtraColor;
    let line = &ctx.props::<DiffLine>().text;
    let background = if line.starts_with('+') && !line.starts_with("+++") {
      ExtraColor::DiffAdded
    } else if line.starts_with('-') && !line.starts_with("---") {
      ExtraColor::DiffRemoved
    } else {
      ExtraColor::CodeBackground
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .background(background.role())
      .child(
        Text::new(line)
          .variant(TypographyStyle::Mono)
          .selectable(true)
          .width(Dimension::Pct(100.0)),
      )
  }
}
