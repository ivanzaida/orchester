//! Project settings / General (02.N `QV1af`): the project's name and folder, its GitHub
//! repository and the account it expects (`yBZWh`), with the notice `V9tFZ`. Renaming and moving
//! the project are not available yet, so name and folder are read-only. The buttons are the
//! form's controls: Tab moves between them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{GitHubConnection, GitHubState, REQUIRED_SCOPE},
  repository::RepositoryLink,
};

use super::{Actions, SettingsScreen, work::SettingsWork};
use crate::{
  connections::data::ProjectConnections,
  display,
  repository::links_same_repository,
  screens::github::detection::{DetectionActions, DetectionBlock, DetectionPlace},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{ValueTone, marked_value_box, value_box_sized},
    form::form,
    icon::Icon,
    notice::Notice,
  },
  workspace::OpenProject,
};

/// Width of the settings content (`vtzYf`), of a setting's value column (`cYbuY`), and the
/// height of its value box and button (`v86BF`, `y4Zv54`).
pub const CONTENT_WIDTH: f32 = 760.0;
const VALUE_WIDTH: f32 = 300.0;
const CONTROL_HEIGHT: f32 = 36.0;
/// Longest folder path shown in full.
const PATH_CHARS: usize = 40;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the General tab is rendered from.
struct General<'a> {
  open: &'a OpenProject,
  data: &'a ProjectConnections,
  actions: &'a Actions,
  /// A check or a change runs: the buttons wait for it.
  busy: bool,
  /// Check again, unless busy.
  check: Option<Handler>,
}

impl SettingsScreen {
  pub(super) fn general(&self, ctx: &Ctx, open: &OpenProject, data: &ProjectConnections, actions: &Actions) -> Element {
    let link = data.link.as_deref();
    let busy = actions.detect.state().get().is_pending() || actions.work.state().get().is_pending();
    let check = (!busy).then(|| {
      let (detect, target) = (actions.detect.clone(), self.target(open, data));
      Arc::new(move || detect.run(target.clone())) as Handler
    });
    let mut content = Column::new()
      .width(Dimension::Px(CONTENT_WIDTH))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("settings.general.project")).variant(ExtraTypography::SectionHeading.role()))
      .child(name_row(ctx, open))
      .child(folder_row(ctx, open))
      .child(repository_row(ctx, link, check.clone()));
    let mut wrong_account = false;
    if let Some(link) = link {
      let connection = data.github_connection(link.connection());
      wrong_account = is_wrong_account(link, connection);
      let use_account = (!busy).then(|| use_account(actions, link, connection)).flatten();
      content = content.child(account_row(ctx, open, link, connection, use_account));
    }
    let general = General {
      open,
      data,
      actions,
      busy,
      check,
    };
    if let Some(block) = self.detection(ctx, &general) {
      content = content.child(block);
    }
    // The notice explains *Use signed-in account*, so it is shown with the button only.
    if wrong_account {
      content = content.child(
        Notice::new(
          Icon::UserCheck,
          ctx.t_args(
            "settings.general.expectedNotice",
            [("project", open.project.name().as_str())],
          ),
          ctx.t("settings.general.expectedNoticeDetail"),
        )
        .id("settings-expected-notice"),
      );
    }
    form(
      None,
      ScrollVertical::new(content)
        .id("settings-general")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
  }

  /// The detected repository block, once a check started (`XcbNT` states in settings).
  fn detection(&self, ctx: &Ctx, general: &General<'_>) -> Option<Element> {
    let General {
      open,
      data,
      actions,
      busy,
      ..
    } = *general;
    let state = actions.detect.state().get();
    if !state.is_pending() && state.data.is_none() {
      return None;
    }
    let detection = state
      .data
      .as_ref()
      .filter(|_| !state.is_pending())
      .map(|detected| &detected.0);
    let link = data.link.as_deref();
    let host_connection = detection.and_then(|detection| detection.github()).and_then(|found| {
      data
        .github
        .iter()
        .find(|connection| connection.host() == found.repo().host())
    });
    let idle = |make: &dyn Fn() -> Handler| (!busy).then(make);
    let linkable = detection
      .filter(|detection| detection.is_linkable() && link.is_none_or(|link| !links_same_repository(detection, link)));
    let block_actions = DetectionActions {
      check: general.check.clone(),
      change_account: idle(&|| {
        let panel = self.panel.clone();
        Arc::new(move || panel.set(true))
      }),
      choose_program: idle(&|| {
        let (program, program_for, title) = (
          actions.program.clone(),
          self.program_for.clone(),
          ctx.t("github.dialog.chooseProgram"),
        );
        let target = host_connection.map(|connection| (connection.id(), connection.name().as_str().to_owned()));
        Arc::new(move || {
          program_for.set(target.clone());
          program.run(title.clone());
        })
      }),
      use_account: link.and_then(|link| {
        let connection = data.github_connection(link.connection());
        (!busy).then(|| use_account(actions, link, connection)).flatten()
      }),
      add_connection: Some({
        let nav = self.nav.clone();
        Arc::new(move || nav.show_add_github())
      }),
      link: linkable.filter(|_| !busy).map(|detection| {
        let (work, expected) = (actions.work.clone(), link.map(|link| link.expected_login().clone()));
        let detection = Box::new(detection.clone());
        Arc::new(move || {
          work.run(SettingsWork::Link {
            detection: detection.clone(),
            expected: expected.clone(),
          });
        }) as Handler
      }),
    };
    let block = DetectionBlock {
      detection,
      expected: link.map(RepositoryLink::expected_login),
      project: open.project.name().as_str(),
      folder: &open.repo_root,
      place: DetectionPlace::Settings,
      copied: &self.copied,
    };
    Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_top(SpacingSize::Sm)
        .child(block.render(ctx, &block_actions))
        .into(),
    )
  }
}

/// *Use signed-in account* while `gh` acts as another account than the link expects.
fn use_account(actions: &Actions, link: &RepositoryLink, connection: Option<&GitHubConnection>) -> Option<Handler> {
  if !is_wrong_account(link, connection) {
    return None;
  }
  let connection = connection?;
  let (work, id, name) = (
    actions.work.clone(),
    connection.id(),
    connection.name().as_str().to_owned(),
  );
  Some(Arc::new(move || {
    work.run(SettingsWork::UseAccount {
      connection: id,
      name: name.clone(),
    });
  }))
}

/// `gh` acts as another account than the link expects: *Use signed-in account* is offered.
fn is_wrong_account(link: &RepositoryLink, connection: Option<&GitHubConnection>) -> bool {
  connection.is_some_and(|connection| {
    matches!(
      connection.state(Some(link.expected_login())),
      GitHubState::WrongAccount { .. }
    )
  })
}

fn name_row(ctx: &Ctx, open: &OpenProject) -> Element {
  let value = value_box_sized(
    ctx,
    "settings-project-name",
    open.project.name().as_str(),
    ValueTone::Value,
    None,
    Dimension::Px(CONTROL_HEIGHT),
  );
  setting_row(
    &ctx.t("settings.general.name"),
    &ctx.t("settings.general.nameDetail"),
    vec![value],
  )
}

fn folder_row(ctx: &Ctx, open: &OpenProject) -> Element {
  let value = marked_value_box(
    ctx,
    "settings-project-folder",
    &display::shorten_path(&open.repo_root, PATH_CHARS),
    (ValueTone::Value, Icon::Folder),
    Dimension::Px(CONTROL_HEIGHT),
  );
  setting_row(
    &ctx.t("settings.general.folder"),
    &ctx.t("settings.general.folderDetail"),
    vec![value, readback(&ctx.t("settings.general.folderReadback"))],
  )
}

/// The linked repository and where it is read from (`dSOKq`), or that none is linked.
fn repository_row(ctx: &Ctx, link: Option<&RepositoryLink>, check: Option<Handler>) -> Element {
  let (value, detail, button) = match link {
    Some(link) => (
      Text::new(&ctx.t_args(
        "github.connection.repositoryName",
        [("owner", link.repo().owner()), ("name", link.repo().name())],
      ))
      .id("settings-repository")
      .variant(TypographyStyle::Body),
      ctx.t_args(
        "settings.general.repositoryReadback",
        [
          ("host", link.repo().host().as_str()),
          ("remote", link.remote().as_str()),
          ("branch", link.default_branch().as_str()),
        ],
      ),
      ("settings-check-repository", ctx.t("github.checkAgain"), Icon::RefreshCw),
    ),
    None => (
      Text::new(&ctx.t("settings.general.notLinked"))
        .id("settings-repository")
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
      ctx.t("settings.general.notLinkedReadback"),
      (
        "settings-detect-repository",
        ctx.t("settings.general.detect"),
        Icon::FolderGit,
      ),
    ),
  };
  setting_row(
    &ctx.t("settings.general.repository"),
    &ctx.t("settings.general.repositoryDetail"),
    vec![value.into(), readback(&detail), small_button(ctx, button, check)],
  )
}

/// The account the project expects, what `gh` is signed in as, and *Use signed-in account*
/// when they differ (`yBZWh`).
fn account_row(
  ctx: &Ctx,
  open: &OpenProject,
  link: &RepositoryLink,
  connection: Option<&GitHubConnection>,
  use_account: Option<Handler>,
) -> Element {
  let state = connection.map(|connection| connection.state(Some(link.expected_login())));
  let detail = match &state {
    Some(GitHubState::SignedIn { account }) => {
      ctx.t_args("settings.general.signedInAs", [("account", account.as_str())])
    }
    Some(GitHubState::WrongAccount { active, .. }) => {
      ctx.t_args("settings.general.signedInAsOther", [("account", active.as_str())])
    }
    Some(GitHubState::MissingScope { account }) => ctx.t_args(
      "settings.general.signedInWithoutScope",
      [("account", account.as_str()), ("scope", REQUIRED_SCOPE)],
    ),
    Some(GitHubState::NotSignedIn) => ctx.t("settings.general.notSignedIn"),
    Some(GitHubState::NotChecked) => ctx.t("settings.general.notChecked"),
    Some(_) | None => ctx.t("settings.general.ghUnavailable"),
  };
  let mut column = vec![
    Text::new(link.expected_login().as_str())
      .id("settings-expected-account")
      .variant(TypographyStyle::Body)
      .into(),
    readback(&detail),
  ];
  if let Some(GitHubState::WrongAccount { active, .. }) = &state {
    let label = ctx.t_args("settings.general.useSignedIn", [("account", active.as_str())]);
    column.push(button_for(
      ctx,
      ("settings-use-signed-in", label, Icon::UserCheck),
      use_account,
    ));
  }
  setting_row(
    &ctx.t("settings.general.expectedAccount"),
    &ctx.t_args(
      "settings.general.expectedAccountDetail",
      [("project", open.project.name().as_str())],
    ),
    column,
  )
}

/// A setting (`l2808`): its name and description, and its value column on the right.
fn setting_row(name: &str, description: &str, value: Vec<Element>) -> Element {
  sized_setting_row(VALUE_WIDTH, name, description, value)
}

/// A setting's name and description beside its value column of `value_width`.
pub(super) fn sized_setting_row(value_width: f32, name: &str, description: &str, value: Vec<Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .overflow_visible()
    .padding_vertical(ExtraSpacing::Divider.role())
    .spacing(SpacingSize::Section)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(name)
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(description).variant(ExtraTypography::Detail.role())),
    )
    .child(
      Column::new()
        .width(Dimension::Px(value_width))
        // A full-width button's focus ring reaches past the column.
        .overflow_visible()
        .spacing(ExtraSpacing::Tight.role())
        .with_children(value),
    )
    .into()
}

/// What a value means now (`fMdmA`, `Toxzz`, `rKHJw`).
pub(super) fn readback(text: &str) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Footnote.role())
    .color(PaletteColor::TextMuted)
    .into()
}

/// A full-width form button (`y4Zv54`), disabled without a handler (while other work runs).
fn button_for(ctx: &Ctx, (id, label, glyph): (&'static str, Arc<str>, Icon), handler: Option<Handler>) -> Element {
  let button = ActionButton::new(label)
    .icon(glyph)
    .id(id)
    .in_form()
    .fill_width()
    .height(Dimension::Px(CONTROL_HEIGHT));
  with_handler(ctx, button, handler)
}

/// A secondary button as wide as its label under a read-only value, e.g. *Check again* in the
/// repository row (`dSOKq`, like `FvVwL` in 01.03).
fn small_button(ctx: &Ctx, (id, label, glyph): (&'static str, Arc<str>, Icon), handler: Option<Handler>) -> Element {
  let button = ActionButton::new(label)
    .icon(glyph)
    .id(id)
    .in_form()
    .height(Dimension::Px(CONTROL_HEIGHT));
  with_handler(ctx, button, handler)
}

fn with_handler(ctx: &Ctx, button: ActionButton, handler: Option<Handler>) -> Element {
  match handler {
    Some(handler) => button.on_click(move || handler()).render(ctx),
    None => button.on_click(|| {}).enabled(false).render(ctx),
  }
}
