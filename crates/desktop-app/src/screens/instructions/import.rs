//! Repository import review U69ob: reads repository files and creates only selected drafts.
use super::state::{Selection, Tab, View};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
  },
};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, VirtualizedList},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  agent_files::import::{Candidate, ImportItem, Unimportable},
  id::IdempotencyKey,
};
use orchester_workspace::{
  background,
  messages::Message,
  skills::repository::{self, CreatedDraft, RepositoryScan},
  tasks::outside::Outside,
};
#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub selection: Selection,
  pub revision: Signal<u64>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
  }
}
impl DevtoolsInspectable for Props {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Imported(Vec<CreatedDraft>);
impl DevtoolsInspectable for Imported {}
#[derive(Clone, Debug, PartialEq, Eq)]
enum ImportError {
  Read,
  Write,
  Background(Message),
}
impl DevtoolsInspectable for ImportError {}
type Action = FutureAction<Vec<ImportItem>, Imported, ImportError>;
pub struct Import {
  selected: Signal<Vec<usize>>,
  preview: Signal<usize>,
  seeded: Signal<bool>,
  action: Action,
}
impl Component for Import {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let key = IdempotencyKey::generate();
    let action = ctx.future_action(move |chosen: Vec<ImportItem>| {
      let root = root.clone();
      async move {
        background::run(move || repository::create_drafts_in(&root, key, &chosen))
          .await
          .map_err(ImportError::Background)?
          .map(Imported)
          .map_err(|error| {
            tracing::error!(%error,"repository import failed");
            ImportError::Write
          })
      }
    });
    let changes = crate::services::Services::of(ctx).changes;
    let id = props.project.open.project.id();
    ctx.watch(&action.state(), move |result| {
      props.selection.busy.set(result.is_pending());
      if result.is_fulfilled() {
        props.revision.update(|v| *v += 1);
        changes.record(id, Outside::Instructions);
      }
    });
    Self {
      selected: ctx.signal(Vec::new()),
      preview: ctx.signal(0),
      seeded: ctx.signal(false),
      action,
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let read = ctx
      .future(root, |root| async move {
        background::run(move || repository::scan(&root))
          .await
          .map_err(ImportError::Background)?
          .map_err(|error| {
            tracing::error!(%error,"repository scan failed");
            ImportError::Read
          })
      })
      .state()
      .get();
    let page = Column::new()
      .id("instruction-import-review")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .padding(ExtraSpacing::Tabs.role())
      .spacing(SpacingSize::Md)
      .child(Text::new(&ctx.t("instructions.import")).variant(TypographyStyle::Title))
      .child(
        Text::new(&ctx.t_args(
          "instructions.importDetail",
          [("folder", props.project.open.repo_root.to_string_lossy().as_ref())],
        ))
        .variant(TypographyStyle::Description),
      );
    if read.is_pending() && read.data.is_none() {
      return page.child(crate::ui::state::loading(
        "instruction-import-loading",
        &ctx.t("instructions.importReading"),
      ));
    }
    if read.is_rejected() {
      return page.child(field_error(
        "instruction-import-error",
        &ctx.t("instructions.importReadFailed"),
      ));
    }
    let Some(scan) = read.data else {
      return page;
    };
    self.loaded(ctx, &props, &scan, page)
  }
}
impl Import {
  fn loaded(&self, ctx: &mut Ctx, props: &Props, scan: &RepositoryScan, mut page: Column) -> Column {
    if !self.seeded.get() {
      self.selected.set(
        scan
          .items
          .iter()
          .enumerate()
          .filter_map(|(i, item)| item.outcome.is_ok().then_some(i))
          .collect(),
      );
      self.seeded.set(true);
    }
    let state = self.action.state().get();
    if let Some(Imported(created)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
      return self.created(ctx, props, created, page);
    }
    if state.is_rejected() {
      page = page.child(field_error(
        "instruction-import-error",
        &ctx.t("instructions.importFailed"),
      ));
    }
    page
      .child(self.review(ctx, scan, state.is_pending()))
      .child(Text::new(&ctx.t("instructions.importNote")).variant(TypographyStyle::Caption))
      .child(self.create_button(ctx, scan, state.is_pending()))
  }
  fn created(&self, ctx: &Ctx, props: &Props, created: &[CreatedDraft], mut page: Column) -> Column {
    page = page.child(
      Text::new(&ctx.t_args("instructions.imported", [("count", created.len().to_string())]))
        .id("instruction-import-result")
        .variant(ExtraTypography::Strong.role()),
    );
    for draft in created {
      page = page.child(Text::new(draft.resource.name().as_str()).variant(TypographyStyle::Description));
    }
    let selection = props.selection.clone();
    page.child(
      ActionButton::new(
        "instruction-import-back",
        ButtonKind::Primary,
        ctx.t("instructions.back"),
      )
      .on_click(move || selection.open(View::Rule(Tab::Project, None))),
    )
  }
  fn review(&self, ctx: &mut Ctx, scan: &RepositoryScan, busy: bool) -> Element {
    let rows = if scan.items.is_empty() {
      Element::from(Text::new(&ctx.t("instructions.importEmpty")).variant(TypographyStyle::Description))
    } else {
      let (selected, preview) = (self.selected.clone(), self.preview.clone());
      VirtualizedList::new_shared(
        ctx,
        std::sync::Arc::new(scan.items.iter().cloned().enumerate().collect::<Vec<_>>()),
      )
      .list_key("instruction-import-rows")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .mount_keyed::<ImportRow, _, _, _>(
        |(_, item)| item.source.clone(),
        move |(index, item)| RowProps {
          index: *index,
          item: item.clone(),
          busy,
          selected: selected.clone(),
          preview: preview.clone(),
        },
      )
    };
    let selected = scan.items.get(self.preview.get());
    let preview = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("instructions.importPreview")).variant(ExtraTypography::Strong.role()))
      .with_children(selected.map(|item| Text::new(&item.source).variant(TypographyStyle::Mono)))
      .with_children(selected.and_then(|item| item.text.as_ref()).map(|text| {
        Text::new(text)
          .variant(crate::theme::instructions::prompt_text())
          .selectable(true)
      }));
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .spacing(SpacingSize::Section)
      .child(
        Column::new()
          .width(Dimension::Px(IMPORT_LIST_WIDTH))
          .height(Dimension::Pct(100.0))
          .min_height(Dimension::Px(0.0))
          .child(rows),
      )
      .child(ScrollVertical::new(preview).flex(1.0).min_height(Dimension::Px(0.0)))
      .into()
  }
  fn create_button(&self, ctx: &Ctx, scan: &RepositoryScan, busy: bool) -> ActionButton {
    let chosen = self
      .selected
      .get()
      .iter()
      .filter_map(|i| scan.items.get(*i).cloned())
      .collect::<Vec<_>>();
    let empty = chosen.is_empty();
    let action = self.action.clone();
    let button = ActionButton::new(
      "instruction-import-create",
      ButtonKind::Primary,
      ctx.t_args("instructions.importCreate", [("count", chosen.len().to_string())]),
    )
    .on_click(move || {
      if !action.state().get_untracked().is_pending() {
        action.run(chosen.clone());
      }
    });
    if busy {
      button.loading()
    } else if empty {
      button.disabled()
    } else {
      button
    }
  }
}
/// Repository item list in U69ob.
const IMPORT_LIST_WIDTH: f32 = 440.0;
#[derive(Clone)]
struct RowProps {
  index: usize,
  item: ImportItem,
  busy: bool,
  selected: Signal<Vec<usize>>,
  preview: Signal<usize>,
}
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.index == other.index && self.item == other.item && self.busy == other.busy
  }
}
impl DevtoolsInspectable for RowProps {}
struct ImportRow;
impl Component for ImportRow {
  type Props = RowProps;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let (index, item, busy) = (props.index, &props.item, props.busy);
    let selected = props.selected.clone();
    let checked = props.selected.get().contains(&index);
    let toggle = crate::ui::toggle::Toggle::new(
      format!("instruction-import-toggle-{index}"),
      crate::ui::toggle::ToggleKind::Checkbox,
      checked,
      item.source.clone(),
    )
    .hide_label();
    let toggle = if item.outcome.is_ok() && !busy {
      toggle.on_change(move |on| {
        selected.update(|chosen| {
          if on && !chosen.contains(&index) {
            chosen.push(index);
          } else if !on {
            chosen.retain(|i| *i != index);
          }
        })
      })
    } else {
      toggle
    };
    let preview = props.preview.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(
        Row::new().width(Dimension::Pct(100.0)).child(toggle).child(
          ActionButton::new(
            format!("instruction-import-item-{index}"),
            ButtonKind::Ghost,
            item.source.clone(),
          )
          .on_click(move || preview.set(index)),
        ),
      )
      .child(Text::new(&item_detail(ctx, item)).variant(TypographyStyle::Caption))
  }
}
pub(super) fn item_detail(ctx: &Ctx, item: &ImportItem) -> std::sync::Arc<str> {
  match &item.outcome {
    Ok(Candidate::Instruction { name, text }) => ctx.t_args(
      "instructions.importInstruction",
      [
        ("name", name.as_str().to_owned()),
        ("lines", text.as_str().lines().count().to_string()),
      ],
    ),
    Ok(Candidate::Skill { name, files, .. }) => ctx.t_args(
      "instructions.importSkill",
      [("name", name.as_str().to_owned()), ("files", files.len().to_string())],
    ),
    Err(reason) => ctx.t(match reason {
      Unimportable::OnlyImports { .. } => "instructions.importOnlyPoints",
      Unimportable::Empty => "instructions.importBlank",
      Unimportable::NotText => "instructions.importNotText",
      Unimportable::NoSkillFile => "instructions.importNoSkill",
      Unimportable::UnsupportedFiles { .. } => "instructions.importUnsupported",
    }),
  }
}
