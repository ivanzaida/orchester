//! The action bar under Add project's form (`Action bar` `YgMOm`, its states `XHoie` in `gh5wV`, `OdlXb`): what
//! is missing or what *Create project & continue* does, and the button. It waits for a folder, for git, and for a
//! folder that is a git repository with a commit; GitHub never holds it back. While the project is created nothing
//! else can be pressed; a creation that failed says why in place of the line, while the name and folder are the
//! ones it was raised for.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::GitHubLogin, repository::Detection};
use orchester_workspace::project_setup::{self, Creation, GitHubLink, ProjectDraft, Readiness, github_connection_name};

use super::{Actions, FORM_WIDTH, Form};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The slot that keeps the error's icon with its first line (`Icon slot` `k3emcM`).
const ERROR_SLOT: f32 = 17.0;

pub(super) fn bar(ctx: &mut Ctx, form: &Form, actions: &Actions, detection: Option<Detection>) -> Element {
  let folder = form.folder.get().0;
  let name = form.name.get();
  let expected = detection.as_ref().and_then(|detection| form.expected_for(detection));
  let state = detection.as_ref().map(|detection| detection.state(expected.as_ref()));
  let local = actions.local(folder.as_deref());
  let readiness = project_setup::readiness(folder.is_some(), state.as_ref(), local.as_ref());
  let created = actions.create.state().get();
  let pending = created.is_pending();
  let draft = ProjectDraft {
    name: &name,
    repository: folder.as_deref(),
  };
  let rejected = created.is_rejected();
  let failure = created
    .error
    .filter(|failure| !pending && rejected && failure.applies_to(&draft));
  let line: Element = match failure {
    Some(failure) => error(&message_text(ctx, &failure.message)),
    None => {
      let (key, color) = match readiness {
        Readiness::NoFolder => ("setup.bar.chooseFolder", PaletteColor::TextMuted),
        _ if actions.git.busy() => ("setup.bar.creatingRepository", PaletteColor::TextMuted),
        Readiness::GitMissing => ("setup.bar.needsGit", PaletteColor::TextMuted),
        Readiness::NotARepository => ("setup.bar.needsRepository", PaletteColor::TextMuted),
        Readiness::NoCommits => ("setup.bar.needsCommit", PaletteColor::TextMuted),
        Readiness::Ready if pending => ("setup.bar.explanation", PaletteColor::TextMuted),
        Readiness::Ready => ("setup.bar.explanation", PaletteColor::TextSecondary),
      };
      Text::new(&ctx.t(key))
        .id("add-project-status")
        .variant(ExtraTypography::Prose.role())
        .color(color)
        .flex(1.0)
        .into()
    }
  };
  let label = if pending {
    "setup.bar.creating"
  } else {
    "setup.bar.create"
  };
  let create = ActionButton::new("create-project", ButtonKind::Primary, ctx.t(label))
    .icon(Icon::ArrowRight)
    .in_form();
  let create = match (pending, readiness) {
    (true, _) => create.loading(),
    (false, Readiness::Ready) => create.on_click(press(ctx, form, actions, (detection, expected))),
    (false, Readiness::NoFolder | Readiness::GitMissing | Readiness::NotARepository | Readiness::NoCommits) => {
      create.disabled()
    }
  };
  Row::new()
    .id("add-project-bar")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xl, SpacingSize::Md))
    .justify(Justify::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Px(FORM_WIDTH))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Center)
        .child(line)
        .child(create),
    )
    .into()
}

/// Create: the name is checked first and said under its field when the rules reject it; otherwise the project
/// is made with the GitHub repository the folder's detection found.
fn press(
  ctx: &Ctx,
  form: &Form,
  actions: &Actions,
  (detection, expected): (Option<Detection>, Option<GitHubLogin>),
) -> impl Fn() + Send + Sync + 'static {
  let connection_name = detection
    .as_ref()
    .and_then(Detection::github)
    .map(|found| message_text(ctx, &github_connection_name(found.repo().host())).to_string());
  let (form, create) = (form.clone(), actions.create.clone());
  move || {
    form.pressed.set(true);
    let name = form.name.get_untracked();
    let folder = form.folder.get_untracked().0;
    let Ok(new) = (ProjectDraft {
      name: &name,
      repository: folder.as_deref(),
    })
    .validate() else {
      return;
    };
    let link = detection
      .clone()
      .zip(connection_name.clone())
      .map(|(detection, connection_name)| GitHubLink {
        detection,
        expected: expected.clone(),
        connection_name,
      });
    create.run(Creation { new, link });
  }
}

/// Why the creation failed, in `danger` (`Error` `juqBM`).
fn error(text: &str) -> Element {
  Row::new()
    .id("add-project-error")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .describe("role", "form-error")
    .child(
      Row::new()
        .height(Dimension::Px(ERROR_SLOT))
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleAlert, IconSize::Base, PaletteColor::Danger)),
    )
    .child(
      Text::new(text)
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::Danger)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
