//! Repository import review U69ob: reads repository files and creates only selected drafts.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  agent_files::import::{Candidate, ImportItem, ItemKind, Unimportable},
  id::IdempotencyKey,
  resource::ResourceKind,
};
use orchester_workspace::{
  background,
  instructions::repository::{self, CreatedDraft, RepositoryScan},
  messages::Message,
  tasks::outside::Outside,
};

use super::{
  import_rows::{Unchosen, Viewed},
  state::{Selection, Tab, View},
};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    icon_button::IconButton,
    notice::{Notice, NoticeTone},
  },
};
#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub selection: Selection,
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
  unchosen: Signal<Unchosen>,
  viewed: Signal<Viewed>,
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
      if let Some(Imported(created)) = result.data.as_ref().filter(|_| result.is_fulfilled()) {
        // Every screen that shows the instructions reads them again, this one too.
        changes.record(id, Outside::Instructions);
        // The Skills screen reads its library again when the import made skill drafts too.
        if created.iter().any(|draft| draft.resource.kind() == ResourceKind::Skill) {
          changes.record(id, Outside::Skills);
        }
      }
    });
    Self {
      unchosen: ctx.signal(Unchosen::default()),
      viewed: ctx.signal(Viewed::default()),
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
      .child(header(ctx, &props));
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
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(
            Padding::new()
              .left(SpacingSize::Xl)
              .right(SpacingSize::Xl)
              .bottom(SpacingSize::Md),
          )
          .child(
            Notice::new(
              "instruction-import-notice",
              NoticeTone::Info,
              ctx.t("instructions.importDraftsOnly"),
            )
            .icon(Icon::ShieldCheck)
            .body(ctx.t("instructions.importNote")),
          ),
      )
      .child(self.footer(ctx, props, scan, state.is_pending()))
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
      super::import_rows::list(ctx, &scan.items, self.unchosen.clone(), self.viewed.clone(), busy)
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(
        Column::new()
          .width(Dimension::Px(IMPORT_LIST_WIDTH))
          .height(Dimension::Pct(100.0))
          .min_height(Dimension::Px(0.0))
          .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
          .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .child(rows),
      )
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .height(Dimension::Pct(100.0))
          .padding(Padding::symmetric(SpacingSize::Xl, ExtraSpacing::Inset.role()))
          .child(super::import_preview::preview(ctx, self.viewed.get().item(&scan.items))),
      )
      .into()
  }
  fn footer(&self, ctx: &Ctx, props: &Props, scan: &RepositoryScan, busy: bool) -> Element {
    let chosen = self.chosen(scan);
    let instructions = chosen.iter().filter(|item| item.kind == ItemKind::Instruction).count();
    let cancel = props.selection.clone();
    let cancel = ActionButton::new(
      "instruction-import-cancel",
      ButtonKind::Secondary,
      ctx.t("instructions.cancel"),
    )
    .on_click(move || cancel.close(View::Rule(Tab::Project, None)));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(IMPORT_FOOTER_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Xl))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Text::new(&ctx.t_args(
          "instructions.importSelection",
          [
            ("selected", chosen.len().to_string()),
            ("total", scan.items.len().to_string()),
            ("instructions", instructions.to_string()),
            ("skills", (chosen.len() - instructions).to_string()),
          ],
        ))
        .id("instruction-import-selection")
        .variant(TypographyStyle::Description)
        .flex(1.0),
      )
      .child(if busy { cancel.disabled() } else { cancel })
      .child(self.create_button(ctx, scan, busy))
      .into()
  }
  fn create_button(&self, ctx: &Ctx, scan: &RepositoryScan, busy: bool) -> ActionButton {
    let chosen = self.chosen(scan);
    let empty = chosen.is_empty();
    let action = self.action.clone();
    let button = ActionButton::new(
      "instruction-import-create",
      ButtonKind::Primary,
      ctx.t_args("instructions.importCreate", [("count", chosen.len().to_string())]),
    )
    .icon(Icon::FilePlus)
    .on_click(move || {
      action.run_if_idle(chosen.clone());
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
impl Import {
  /// The items the import makes drafts of, in the scan's order.
  fn chosen(&self, scan: &RepositoryScan) -> Vec<ImportItem> {
    let unchosen = self.unchosen.get();
    scan
      .items
      .iter()
      .filter(|item| unchosen.chosen(item))
      .cloned()
      .collect()
  }
}
/// Items pane qNGoR and footer WLlV8 in the import review.
const IMPORT_LIST_WIDTH: f32 = 460.0;
const IMPORT_FOOTER_HEIGHT: f32 = 56.0;

fn header(ctx: &mut Ctx, props: &Props) -> Element {
  let back = props.selection.clone();
  let crumbs = breadcrumb(
    ctx,
    "instruction-import-crumbs",
    CrumbSeparator::Chevron,
    vec![
      Crumb::link(ctx.t("instructions.title"), move || {
        back.open(View::Rule(Tab::Project, None))
      }),
      Crumb::here(ctx.t("instructions.import")),
    ],
  );
  let back = props.selection.clone();
  let back = IconButton::new(
    "instruction-import-return",
    Icon::ArrowLeft,
    ctx.t("instructions.backInstructions"),
  )
  .small()
  .on_click(move || back.open(View::Rule(Tab::Project, None)))
  .render(ctx);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xl, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(back)
        .child(crumbs),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t("instructions.import")).variant(TypographyStyle::Heading))
        .child(
          Text::new(&ctx.t_args(
            "instructions.importDetail",
            [("folder", props.project.open.repo_root.to_string_lossy().as_ref())],
          ))
          .variant(TypographyStyle::Description),
        ),
    )
    .into()
}
pub(super) fn item_detail(ctx: &Ctx, item: &ImportItem) -> std::sync::Arc<str> {
  match &item.outcome {
    Err(Unimportable::OnlyImports { targets }) => ctx.t_args(
      "instructions.importOnlyPoints",
      [
        ("targets", targets.join(", ")),
        (
          "references",
          targets
            .iter()
            .map(|target| format!("@{target}"))
            .collect::<Vec<_>>()
            .join(", "),
        ),
      ],
    ),
    Ok(Candidate::Instruction { name, text }) => ctx.t_args(
      "instructions.importInstruction",
      [
        ("name", name.as_str().to_owned()),
        ("lines", text.as_str().lines().count().to_string()),
      ],
    ),
    Ok(Candidate::Skill { name, files, .. }) => ctx.t_args(
      match files.len() {
        0 => "instructions.importSkillNone",
        1 => "instructions.importSkillOne",
        _ => "instructions.importSkill",
      },
      [("name", name.as_str().to_owned()), ("files", files.len().to_string())],
    ),
    Err(Unimportable::Empty) => ctx.t("instructions.importBlank"),
    Err(Unimportable::NotText) => ctx.t("instructions.importNotText"),
    Err(Unimportable::NoSkillFile) => ctx.t("instructions.importNoSkill"),
    Err(Unimportable::UnsupportedFiles { .. }) => ctx.t("instructions.importUnsupported"),
  }
}
