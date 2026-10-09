//! Import from repository (ORC-15 `wulRC` review, `Z8mGB0` result): what the repository gives its
//! agents — `AGENTS.md`, `CLAUDE.md` and its skills folders — each with what it becomes and a
//! preview, the chosen ones created as drafts, then what was made and what was not. It opens over
//! the page it was opened from (Skills, Instructions, Project settings), as its path says
//! ([`crate::navigation::repository_import`]). It only reads the repository.

pub(super) mod copy;
mod items;
mod result;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  agent_files::import::{ImportItem, ItemKind},
  id::IdempotencyKey,
};

use self::result::Created;
use super::ProjectNav;
use crate::{
  activity::{OperationSpec, skill_operations},
  background,
  messages::Message,
  navigation::repository_import as query,
  skills::repository::{RepositoryScan, create_drafts_in, scan},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::{ActionButton, SKIPPED_BY_TAB},
    form::form,
    icon::Icon,
    message_text,
    notice::Notice,
    panel::RecoveryPanel,
  },
  workspace::OpenProject,
};

/// Dialog sizes, the footer's height and its buttons' (`Hljtt`, `MyH5Q`, `nNVS3`; `rcock`).
const DIALOG_WIDTH: f32 = 1040.0;
const DIALOG_HEIGHT: f32 = 780.0;
const RESULT_WIDTH: f32 = 600.0;
const FOOTER_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

/// The dialog over the page the path names, while the path carries it; nothing otherwise.
pub(super) fn dialog(ctx: &mut Ctx, project: &OpenProject) -> Element {
  ctx.mount_keyed::<RepositoryImportLayer>("repository-import-layer", project.clone())
}

/// The modal layer of Import from repository, open while the path carries the dialog; Escape
/// closes it like every other dialog, which leaves the dialog's history entry.
pub struct RepositoryImportLayer {
  open: Signal<bool>,
}

impl Component for RepositoryImportLayer {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let open = ctx.signal(false);
    ctx.watch(&open, move |open| {
      if !*open && nav.repository_import_shown() {
        nav.close_repository_import();
      }
    });
    Self { open }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let shown = query::shown(&ctx.route_path());
    // Follows the path, so that only Escape turns it off while the path still carries the dialog.
    if self.open.get_untracked() != shown {
      self.open.set(shown);
    }
    if !shown {
      return Modal::new(Column::new()).open_when(false);
    }
    let dialog = ctx.mount_keyed::<RepositoryImportDialog>("repository-import", ctx.props::<OpenProject>().clone());
    Modal::new(backdrop(ctx, dialog))
      .target(ModalTarget::Root)
      .open(self.open.clone())
  }
}

/// The sources chosen to become drafts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Chosen(Vec<String>);

impl lurq::app::component::DevtoolsInspectable for Chosen {}

/// The source whose preview shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Opened(Option<String>);

impl lurq::app::component::DevtoolsInspectable for Opened {}

type CreateArgs = (PathBuf, IdempotencyKey, Vec<ImportItem>, OperationSpec);
type CreateAction = FutureAction<CreateArgs, Created, Message>;

pub struct RepositoryImportDialog {
  chosen: Signal<Chosen>,
  opened: Signal<Opened>,
  /// The choice starts with every importable item, once they are read.
  seeded: Signal<bool>,
  /// Identifies this import, so retrying after a failure creates nothing twice.
  key: IdempotencyKey,
  /// Focused when the dialog opens, so Tab starts at its first control.
  frame: ElementRef,
  nav: ProjectNav,
}

impl Component for RepositoryImportDialog {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let action = create_action(ctx);
    let changed = nav.clone();
    ctx.watch(&action.state(), move |state| {
      if state.data.is_some() && !state.is_pending() {
        changed.resources_changed();
      }
    });
    let dialog = Self {
      chosen: ctx.signal(Chosen::default()),
      opened: ctx.signal(Opened::default()),
      seeded: ctx.signal(false),
      key: IdempotencyKey::generate(),
      frame: ElementRef::new(),
      nav,
    };
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let action = create_action(ctx);
    let load = ctx.future(open.repo_root.clone(), |repo_root| async move {
      let root = repo_root.clone();
      background::run(move || scan(&root)).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the repository's agent files");
        Message::new("skills.import.readFailed").with("folder", repo_root.display().to_string())
      })
    });
    let state = load.state().get();
    let created = action.state().get();
    if let (Some(created), Some(scan)) = (created.data.as_ref().filter(|_| !created.is_pending()), &state.data) {
      let dialog = self.frame(RESULT_WIDTH, None, result::result(ctx, created, scan, &self.nav));
      return form(None, dialog);
    }
    let body = match (&state.data, &state.error) {
      (_, Some(error)) => centered(RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("skills.import.readFailedTitle"),
        &message_text(ctx, error),
      )),
      (Some(scan), None) if scan.items.is_empty() => centered(RecoveryPanel::new(
        Icon::FolderGit,
        &ctx.t("skills.import.nothingTitle"),
        &ctx.t("skills.import.nothingDetail"),
      )),
      (Some(scan), None) => {
        self.seed(scan);
        self.review(ctx, scan)
      }
      (None, None) => centered(RecoveryPanel::new(
        Icon::FolderSearch,
        &ctx.t("skills.import.reading"),
        &ctx.t("skills.import.readingDetail"),
      )),
    };
    let chosen = state
      .data
      .as_ref()
      .map_or_else(Vec::new, |scan| self.chosen_items(scan));
    let create = self.create(&open, &chosen, &action);
    let footer = self.footer(ctx, state.data.as_ref(), &chosen, (&action, create.clone()));
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(header(ctx, &open))
      .child(body)
      .child(notice(ctx))
      .child(footer);
    let dialog = self.frame(DIALOG_WIDTH, Some(DIALOG_HEIGHT), page.into());
    form(create, dialog)
  }
}

type Handler = Arc<dyn Fn() + Send + Sync>;

impl RepositoryImportDialog {
  fn frame(&self, width: f32, height: Option<f32>, content: Element) -> Element {
    let mut dialog = Column::new()
      .id("repository-import-dialog")
      .ref_element(self.frame.clone())
      .focusable(true)
      .tab_index(SKIPPED_BY_TAB)
      .width(Dimension::Px(width))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(content);
    if let Some(height) = height {
      dialog = dialog.height(Dimension::Px(height));
    }
    dialog.into()
  }

  /// Every importable item is chosen, and the first item's preview shows, once they are read.
  fn seed(&self, scan: &RepositoryScan) {
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    let importable = scan
      .items
      .iter()
      .filter(|item| item.outcome.is_ok())
      .map(|item| item.source.clone())
      .collect();
    self.chosen.set(Chosen(importable));
    self
      .opened
      .set(Opened(scan.items.first().map(|item| item.source.clone())));
  }

  /// Creating the `chosen` drafts, unless nothing is chosen or they are being created; Create N
  /// drafts and Enter run it.
  fn create(&self, open: &OpenProject, chosen: &[ImportItem], action: &CreateAction) -> Option<Handler> {
    if chosen.is_empty() || action.state().get().is_pending() {
      return None;
    }
    let (run, repo_root, key, chosen) = (action.clone(), open.repo_root.clone(), self.key, chosen.to_vec());
    let spec = skill_operations::repository_import(open, chosen.len());
    Some(Arc::new(move || {
      run.run((repo_root.clone(), key, chosen.clone(), spec.clone()))
    }))
  }

  /// The count of what is chosen, Cancel and Create N drafts (`MyH5Q`).
  fn footer(
    &self,
    ctx: &Ctx,
    scan: Option<&RepositoryScan>,
    chosen: &[ImportItem],
    (action, create): (&CreateAction, Option<Handler>),
  ) -> Element {
    let running = action.state().get();
    let count = chosen.len();
    let mut children: Vec<Element> = Vec::new();
    match running.error.as_ref().filter(|_| !running.is_pending()) {
      Some(error) => children.push(crate::ui::field::field_error(&message_text(ctx, error))),
      None => {
        if let Some(scan) = scan.filter(|scan| !scan.items.is_empty()) {
          children.push(
            Text::new(&selection(ctx, scan, chosen))
              .variant(TypographyStyle::Caption)
              .id("repository-import-count")
              .into(),
          );
        }
      }
    }
    children.push(Spacer::new().flex(1.0).into());
    let nav = self.nav.clone();
    children.push(
      ActionButton::new(ctx.t("common.cancel"))
        .id("repository-import-cancel")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || nav.close_repository_import())
        .render(ctx),
    );
    let label = if count == 1 {
      ctx.t("skills.import.create.one")
    } else {
      ctx.t_args("skills.import.create.other", [("count", count.to_string().as_str())])
    };
    let button = ActionButton::new(label)
      .primary()
      .icon(Icon::FilePlus)
      .id("repository-import-create")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT));
    let button = match create {
      Some(create) => button.on_click(move || create()),
      None => button.on_click(|| {}).enabled(false),
    };
    children.push(button.render(ctx));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FOOTER_HEIGHT))
      .padding_horizontal(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .with_children(children)
      .into()
  }

  /// The chosen items that can be imported, in list order.
  fn chosen_items(&self, scan: &RepositoryScan) -> Vec<ImportItem> {
    let Chosen(chosen) = self.chosen.get();
    scan
      .items
      .iter()
      .filter(|item| item.outcome.is_ok() && chosen.contains(&item.source))
      .cloned()
      .collect()
  }
}

/// "4 of 6 selected · 1 instruction, 3 skills".
fn selection(ctx: &Ctx, scan: &RepositoryScan, chosen: &[ImportItem]) -> std::sync::Arc<str> {
  let of_kind = |kind| chosen.iter().filter(|item| item.kind == kind).count();
  let counted = |count: usize, one: &str, other: &str| {
    if count == 1 {
      ctx.t(one)
    } else {
      ctx.t_args(other, [("count", count.to_string().as_str())])
    }
  };
  let instructions = counted(
    of_kind(ItemKind::Instruction),
    "skills.import.instructions.one",
    "skills.import.instructions.other",
  );
  let skills = counted(
    of_kind(ItemKind::Skill),
    "skills.import.skills.one",
    "skills.import.skills.other",
  );
  ctx.t_args(
    "skills.import.selected",
    [
      ("selected", chosen.len().to_string().as_str()),
      ("total", scan.items.len().to_string().as_str()),
      ("instructions", &*instructions),
      ("skills", &*skills),
    ],
  )
}

/// Title and what the dialog does (`AtE81`).
fn header(ctx: &Ctx, open: &OpenProject) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t("skills.import.title"))
        .variant(ExtraTypography::DialogTitle.role())
        .id("repository-import-title"),
    )
    .child(
      Text::new(&ctx.t_args(
        "skills.import.explanation",
        [("folder", open.repo_root.display().to_string().as_str())],
      ))
      .variant(ExtraTypography::Explanation.role()),
    )
    .into()
}

/// Creates drafts only (`i70SHm`).
fn notice(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Stack.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Notice::new(
      Icon::ShieldCheck,
      ctx.t("skills.import.notice"),
      ctx.t("skills.import.noticeDetail"),
    ))
    .into()
}

fn centered(panel: RecoveryPanel) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(panel)
    .into()
}

/// Creates the chosen drafts, reported in the status bar.
fn create_action(ctx: &mut Ctx) -> CreateAction {
  let activity = crate::activity::of(ctx);
  ctx.future_action(move |(repo_root, key, chosen, spec): CreateArgs| {
    let activity = activity.clone();
    async move {
      let work = async move {
        background::run(move || create_drafts_in(&repo_root, key, &chosen))
          .await?
          .map(Created)
          .map_err(|error| {
            tracing::error!(%error, "cannot create drafts from the repository");
            Message::new("skills.import.createFailed")
          })
      };
      activity.track(spec, work).await
    }
  })
}
