//! Project secrets (07.01 `R99lt`): the project's secrets beside the selected one's details
//! (`CQxCb`), adding one (07.02 `tpF7R`; a name already taken, 07.A `m9Dhp`), replacing its value
//! and archiving it. A value is typed into a masked field, goes straight to the OS keychain and is
//! never shown again: the design offers Replace value, no reveal. Import from project, the usage
//! log and the audit log do not exist yet and are shown unavailable; which tools use a secret is
//! derived from the tools.

mod archive_dialog;
mod dialog;
mod fields;
mod inspector;
pub(super) mod labels;
mod library;
mod row;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Text},
  core::Signal,
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::SecretId,
  secret::{Environment, Secret},
};

use self::{
  archive_dialog::{ArchiveDialog, archive_dialog},
  dialog::{DialogPurpose, SecretDialog, SecretDialogContext},
};
use super::{
  ProjectNav,
  page::{self, TOPBAR_BUTTON_HEIGHT},
};
use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  messages::{self, Message},
  navigation::route::{Page, Route},
  secrets::{
    Vault,
    data::{ProjectSecrets, archive_in},
  },
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Tab row height (`y0NYIU`).
const TAB_ROW_HEIGHT: f32 = 42.0;

/// Which dialog is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogMode {
  Closed,
  Add,
  Replace(Secret),
  Archive(Secret),
}

impl lurq::app::component::DevtoolsInspectable for DialogMode {}

/// The environment the list shows; `None` shows every environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironmentFilter(pub Option<Environment>);

impl lurq::app::component::DevtoolsInspectable for EnvironmentFilter {}

/// The archived secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Archived(SecretId);

impl lurq::app::component::DevtoolsInspectable for Archived {}

type ArchiveAction = FutureAction<(PathBuf, Vault, SecretId, OperationSpec), Archived, Message>;

enum Loaded {
  Loading,
  Ready(ProjectSecrets),
  Failed(Message),
}

pub struct SecretsScreen {
  dialog: Signal<DialogMode>,
  filter: Signal<EnvironmentFilter>,
  /// Bumped after a change, to read the secrets again.
  revision: Signal<u64>,
  nav: ProjectNav,
  vault: Vault,
  /// Where the list and the details are scrolled: kept by the screen, so a tab that goes
  /// inactive and back keeps them.
  list_scroll: ScrollState,
  details_scroll: ScrollState,
}

impl Component for SecretsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let vault = ctx
      .use_context::<Vault>()
      .expect("the root provides the Vault before mounting screens");
    let screen = Self {
      dialog: ctx.signal(DialogMode::Closed),
      filter: ctx.signal(EnvironmentFilter(None)),
      revision: ctx.signal(0_u64),
      nav,
      vault,
      list_scroll: ScrollState::new(),
      details_scroll: ScrollState::new(),
    };
    let archive = archive_action(ctx);
    let (dialog, revision, nav) = (screen.dialog.clone(), screen.revision.clone(), screen.nav.clone());
    ctx.watch(&archive.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        dialog.set(DialogMode::Closed);
        revision.update(|revision| *revision += 1);
        // The archived secret is gone from the list; the first one is shown in its place.
        nav.replace(Page::SECRETS);
      }
    });
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    let selected = match Route::current_page(ctx) {
      Some(Page::Secrets(selected)) => selected,
      _ => None,
    };
    // Futures are matched by call order: the action comes first, as in `create`.
    let archive = archive_action(ctx);
    let loaded = self.load(ctx, &open);
    let body = match &loaded {
      Loaded::Loading => Column::new().flex(1.0).into(),
      Loaded::Failed(error) => centered(
        RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("secrets.loadFailed"),
          &message_text(ctx, error),
        )
        .into(),
      ),
      Loaded::Ready(data) => self.library(ctx, &open, data, selected),
    };
    let breadcrumb = ctx.t_args("secrets.breadcrumb", [("project", open.project.name().as_str())]);
    let import = ActionButton::new(ctx.t("secrets.import"))
      .icon(Icon::Download)
      .id("secrets-import")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
      .render(ctx);
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, [import, self.add_button(ctx, "add-secret")]))
      .child(heading(ctx))
      .child(tabs(ctx))
      .child(body)
      .child(self.dialog(ctx, &open, &loaded, &archive))
  }
}

impl SecretsScreen {
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> Loaded {
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, _)| async move {
      background::run(move || ProjectSecrets::load_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's secrets");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Loaded::Failed(error),
      (Some(data), None) => Loaded::Ready(data),
      (None, None) => Loaded::Loading,
    }
  }

  fn add_button(&self, ctx: &Ctx, id: &'static str) -> Element {
    let dialog = self.dialog.clone();
    ActionButton::new(ctx.t("secrets.add"))
      .primary()
      .icon(Icon::Plus)
      .id(id)
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
      .on_click(move || dialog.set(DialogMode::Add))
      .render(ctx)
  }

  /// The open dialog over a backdrop: add or replace (`E213dn`), or the archive confirmation.
  fn dialog(&self, ctx: &mut Ctx, open: &OpenProject, loaded: &Loaded, archive: &ArchiveAction) -> Element {
    let (Loaded::Ready(data), mode) = (loaded, self.dialog.get()) else {
      return Modal::new(Column::new()).open_when(false).into();
    };
    let content = match mode {
      DialogMode::Closed => return Modal::new(Column::new()).open_when(false).into(),
      DialogMode::Add => self.secret_dialog(ctx, open, data, DialogPurpose::Add),
      DialogMode::Replace(secret) => self.secret_dialog(ctx, open, data, DialogPurpose::Replace(secret)),
      DialogMode::Archive(secret) => {
        let uses = data.used_by(secret.id()).count();
        self.archive_dialog(ctx, open, archive, &secret, uses)
      }
    };
    Modal::new(content)
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }

  fn secret_dialog(&self, ctx: &mut Ctx, open: &OpenProject, data: &ProjectSecrets, purpose: DialogPurpose) -> Element {
    let key = match &purpose {
      DialogPurpose::Add => "secret-dialog-add".to_owned(),
      DialogPurpose::Replace(secret) => format!("secret-dialog-{}", secret.id()),
    };
    let (close, saved, show) = (self.dialog.clone(), self.dialog.clone(), self.dialog.clone());
    let (nav, show_nav, revision) = (self.nav.clone(), self.nav.clone(), self.revision.clone());
    ctx.provide(SecretDialogContext {
      project: open.clone(),
      vault: self.vault.clone(),
      purpose,
      secrets: data.secrets.clone(),
      environment: self.filter.get().0,
      close: Arc::new(move || close.set(DialogMode::Closed)),
      saved: Arc::new(move |id| {
        // The saved secret is shown in place of the one shown before.
        nav.replace(Page::Secrets(Some(id)));
        revision.update(|revision| *revision += 1);
        saved.set(DialogMode::Closed);
      }),
      show: Arc::new(move |id| {
        show.set(DialogMode::Closed);
        show_nav.show(Page::Secrets(Some(id)));
      }),
    });
    ctx.mount_keyed::<SecretDialog>(&key, ())
  }

  fn archive_dialog(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    archive: &ArchiveAction,
    secret: &Secret,
    uses: usize,
  ) -> Element {
    let state = archive.state().get();
    let pending = state.is_pending();
    let confirm = (!pending).then(|| {
      let (archive, vault) = (archive.clone(), self.vault.clone());
      let args = (
        open.repo_root.clone(),
        secret.id(),
        operations::secret_archive(open, secret.name().as_str()),
      );
      Arc::new(move || archive.run((args.0.clone(), vault.clone(), args.1, args.2.clone())))
        as Arc<dyn Fn() + Send + Sync>
    });
    let close = self.dialog.clone();
    archive_dialog(
      ctx,
      &ArchiveDialog {
        secret,
        project: open.project.name().as_str(),
        uses,
        pending,
        error: state.error.filter(|_| !pending),
        confirm,
        cancel: Arc::new(move || close.set(DialogMode::Closed)),
      },
    )
  }
}

/// Archives a secret off the UI thread, tracked as the operation it is given.
fn archive_action(ctx: &mut Ctx) -> ArchiveAction {
  let activity = Activity::of(ctx);
  ctx.future_action(
    move |(repo_root, vault, id, spec): (PathBuf, Vault, SecretId, OperationSpec)| {
      let activity = activity.clone();
      async move {
        let archive = async move {
          let archived = background::run(move || archive_in(&repo_root, vault.get(), id)).await?;
          archived.map(|()| Archived(id)).map_err(|error| {
            tracing::error!(%error, "cannot archive the secret");
            messages::secret_error(&error)
          })
        };
        activity.track(spec, archive).await
      }
    },
  )
}

/// Title and description (`f5bMeF`): the screen title over a 13px description.
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("secrets.title")).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&ctx.t("secrets.description"))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}

/// Secrets and Usage log (`y0NYIU`); the usage log does not exist yet.
fn tabs(ctx: &Ctx) -> Element {
  let height = Dimension::Pct(100.0);
  tab_row(
    Dimension::Px(TAB_ROW_HEIGHT),
    ExtraSpacing::Step.role(),
    Some(SpacingSize::Section),
    [
      Tab::new("secrets-tab-secrets", ctx.t("secrets.tabs.secrets"), height).selected(true),
      Tab::new("secrets-tab-usage", ctx.t("secrets.tabs.usageLog"), height),
    ],
  )
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
