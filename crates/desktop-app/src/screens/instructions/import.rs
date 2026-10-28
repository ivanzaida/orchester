//! Import from repository (`02c` `P21bwh`), one page under the hub's tabs reached from Instructions and Skills: the
//! crumb, the page's title and where it read, the notice that it only creates drafts, then one group per kind with a
//! row per item found, and the bar with what is chosen, Cancel and Create; beside them the opened item's preview;
//! then what it created (`03c` `cC55E`). It reads repository files and creates only the chosen drafts.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  agent_files::{
    RepositoryAgentFiles,
    import::{Candidate, ImportItem, ItemKind, Unimportable},
  },
  id::IdempotencyKey,
  resource::ResourceKind,
};
use orchester_workspace::{
  background,
  instructions::repository::{self, CreatedDraft, RepositoryScan},
  messages::Message,
  project_settings,
  skills::repository::ImportFailure,
  tasks::outside::Outside,
};

use super::state::Selection;
use crate::{
  project::ShownProject,
  screens::{
    hub::{
      self,
      draft_bar::{DraftBar, Standing},
      import_preview, import_result,
      import_rows::{Review, Unchosen, Viewed},
      page_head,
    },
    skills::words,
  },
  theme::{ExtraSpacing, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
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
/// The drafts an import created, of how many items, and the project's Repository agent files when they could be read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Imported(Vec<CreatedDraft>, usize, Option<RepositoryAgentFiles>);
impl DevtoolsInspectable for Imported {}
#[derive(Clone, Debug, PartialEq, Eq)]
enum ImportError {
  Read,
  Write,
  Background(Message),
}
impl DevtoolsInspectable for ImportError {}
type Action = FutureAction<(Vec<ImportItem>, usize), Imported, ImportError>;
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
    let action = ctx.future_action(move |(chosen, total): (Vec<ImportItem>, usize)| {
      let root = root.clone();
      async move {
        background::run(move || {
          let created = repository::create_drafts_in(&root, key, &chosen)?;
          // The result says whether the repository's own files still reach agents; a failed read leaves that out.
          let files = project_settings::load_in(&root)
            .inspect_err(|error| tracing::warn!(%error, "repository agent files could not be read"))
            .ok()
            .map(|settings| settings.agent_files);
          Ok::<_, ImportFailure>(Imported(created, total, files))
        })
        .await
        .map_err(ImportError::Background)?
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
      if let Some(Imported(created, ..)) = result.data.as_ref().filter(|_| result.is_fulfilled()) {
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
    let mut page = Column::new()
      .id("instruction-import-review")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(top(ctx, &props));
    if read.is_pending() && read.data.is_none() {
      page = page.child(crate::ui::state::loading(
        "instruction-import-loading",
        &ctx.t("instructions.importReading"),
      ));
      return hub::item_body(PAGE, page, None);
    }
    if read.is_rejected() {
      page = page.child(field_error(
        "instruction-import-error",
        &ctx.t("instructions.importReadFailed"),
      ));
      return hub::item_body(PAGE, page, None);
    }
    let Some(scan) = read.data else {
      return hub::item_body(PAGE, page, None);
    };
    self.loaded(ctx, &props, &scan, page)
  }
}
/// The page's scrolling body (`instruction-import-page`, its `-pane` and `-side`).
const PAGE: &str = "instruction-import-page";
impl Import {
  fn loaded(&self, ctx: &mut Ctx, props: &Props, scan: &RepositoryScan, mut page: Column) -> Element {
    let state = self.action.state().get();
    if let Some(Imported(created, total, files)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
      let back = &props.selection;
      let list = ctx.t(back.opened_from_label());
      let result = import_result::Outcome {
        root: "instruction-import-result",
        id: "instruction-import",
        project: props.project.open.project.id(),
        back: (&list, back.to(back.opened_from())),
        created,
        items: &scan.items,
        total: *total,
        agent_files: *files,
      }
      .page(ctx);
      return hub::item_body(PAGE, result, None);
    }
    page = page.child(
      Notice::new(
        "instruction-import-notice",
        NoticeTone::Info,
        ctx.t("instructions.importDraftsOnly"),
      )
      .icon(Icon::Info)
      .body(ctx.t("instructions.importNote")),
    );
    if state.is_rejected() {
      page = page.child(field_error(
        "instruction-import-error",
        &ctx.t("instructions.importFailed"),
      ));
    }
    let busy = state.is_pending();
    page = if scan.items.is_empty() {
      page.child(
        Text::new(&ctx.t("instructions.importEmpty"))
          .id("instruction-import-empty")
          .variant(HubText::Hint.role()),
      )
    } else {
      let review = Review {
        id: "instruction-import",
        items: &scan.items,
        unchosen: self.unchosen.clone(),
        viewed: self.viewed.clone(),
        busy,
        detail: item_detail,
      };
      page.with_children(review.groups(ctx))
    };
    page = page.child(self.footer(ctx, props, scan, busy));
    let side = import_preview::preview(
      ctx,
      "instruction-import",
      self.viewed.get().item(&scan.items),
      item_detail,
    );
    hub::item_body(PAGE, page, Some(side))
  }
  /// What is chosen, then Cancel and Create (`Draft bar` `KxTuA`).
  fn footer(&self, ctx: &Ctx, props: &Props, scan: &RepositoryScan, busy: bool) -> Element {
    let chosen = self.chosen(scan);
    let instructions = chosen.iter().filter(|item| item.kind == ItemKind::Instruction).count();
    let cancel = props.selection.clone();
    let cancel = ActionButton::new(
      "instruction-import-cancel",
      ButtonKind::Ghost,
      ctx.t("instructions.cancel"),
    )
    .on_click(move || cancel.close(cancel.opened_from()));
    let status = ctx.t_args(
      "skills.importSelected",
      [
        ("selected", chosen.len().to_string()),
        ("total", scan.items.len().to_string()),
        (
          "types",
          words::types(ctx, instructions, chosen.len() - instructions).to_string(),
        ),
      ],
    );
    // `instruction-import-selection`: the bar keeps the id the import gave what is chosen.
    DraftBar::new("instruction-import-selection", Standing::Choosing, status)
      .action(if busy { cancel.disabled() } else { cancel })
      .action(self.create_button(ctx, scan, busy))
      .into()
  }
  fn create_button(&self, ctx: &Ctx, scan: &RepositoryScan, busy: bool) -> ActionButton {
    let chosen = self.chosen(scan);
    let empty = chosen.is_empty();
    let action = self.action.clone();
    let total = scan.items.len();
    let button = ActionButton::new(
      "instruction-import-create",
      ButtonKind::Primary,
      words::create_drafts(ctx, chosen.len()),
    )
    .icon(Icon::FilePlus)
    .on_click(move || {
      action.run_if_idle((chosen.clone(), total));
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
    self.unchosen.get().of(&scan.items)
  }
}
/// The crumb back to Instructions, then the page's title and where it read (`Top` `F9FVc`).
fn top(ctx: &Ctx, props: &Props) -> Element {
  let back = &props.selection;
  let title = ctx.t("instructions.import");
  let crumb = page_head::crumb(
    ctx,
    "instruction-import-crumbs",
    (&ctx.t(back.opened_from_label()), back.to(back.opened_from())),
    Some(&title),
  );
  let detail = ctx.t_args(
    "instructions.importDetail",
    [("folder", props.project.open.repo_root.to_string_lossy().as_ref())],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Row.role())
    .child(crumb)
    .child(
      Column::new()
        .id("instruction-import-head")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(&title)
            .id("instruction-import-title")
            .variant(HubText::PageTitle.role()),
        )
        .child(
          Text::new(&detail)
            .id("instruction-import-sub")
            .variant(HubText::Hint.role())
            .color(PaletteColor::TextSecondary)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}
/// What an item becomes, or why it cannot be imported (`Draft project instruction “AGENTS” · 84 lines`), the same
/// whether Instructions or Skills opened the import (02c).
pub(crate) fn item_detail(ctx: &Ctx, item: &ImportItem) -> std::sync::Arc<str> {
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
    Ok(Candidate::Instruction { name, text }) => {
      let lines = text.as_str().lines().count();
      ctx.t_args(
        if lines == 1 {
          "instructions.importInstructionOne"
        } else {
          "instructions.importInstruction"
        },
        [("name", name.as_str().to_owned()), ("lines", lines.to_string())],
      )
    }
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
