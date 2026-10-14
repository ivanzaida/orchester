//! *Change GitHub account or access* in place in the GitHub block (`Change GitHub account or access` `XKOXY` in
//! `vJTcd`): the host, the account `gh` is signed in to, its scopes and the account the project expects, then the
//! `gh` commands that sign in, switch accounts or add the `repo` scope, each with Copy, and Close and *Check
//! again*. Orchester never runs them, never signs in, switches accounts or sees a token.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{GhAdvice, GitHubCheckResult, GitHubHost, REQUIRED_SCOPE},
  repository::GitHubDetection,
};

use super::{
  super::{Actions, Form},
  command_row, property,
  words::{Property, Value},
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    icon_button::IconButton,
  },
};

/// The line between the commands and the actions (`Divider` `KU9Lf`).
const DIVIDER: f32 = 1.0;

/// The panel for the `gh` that `found` asked; `project` is the name the project will have.
pub(super) fn panel(ctx: &mut Ctx, form: &Form, actions: &Actions, found: &GitHubDetection, project: &str) -> Element {
  let host = found.repo().host().clone();
  let close = form.access.clone();
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(&ctx.t("setup.github.access.title")).variant(ProjectsText::Heading.role()))
        .child(Text::new(&ctx.t("setup.github.access.detail")).variant(ExtraTypography::Prose.role())),
    )
    .child(
      IconButton::new("github-access-dismiss", Icon::X, ctx.t("setup.github.access.close"))
        .small()
        .on_click(move || close.set(false))
        .render(ctx),
    );
  let other = section(
    ctx,
    ("setup.github.access.otherTitle", "setup.github.access.otherDetail"),
    &[
      ("github-access-login", GhAdvice::Login),
      ("github-access-switch", GhAdvice::Switch(None)),
    ],
    &host,
  );
  let access = section(
    ctx,
    ("setup.github.access.accessTitle", "setup.github.access.accessDetail"),
    &[("github-access-refresh", GhAdvice::Refresh)],
    &host,
  );
  Column::new()
    .id("github-access")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inset.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Tile.role())
    .child(head)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .with_children(current(ctx, form, found, project).into_iter().map(property)),
    )
    .child(other)
    .child(access)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(DIVIDER))
        .background(PaletteColor::Border),
    )
    .child(bottom(ctx, form, actions))
    .into()
}

/// The host, the account `gh` is signed in to, its scopes and the account the project expects (`Current` `Ikk1G`).
fn current(ctx: &Ctx, form: &Form, found: &GitHubDetection, project: &str) -> Vec<Property> {
  let result = found.check().map(|check| &check.result);
  let mut current = vec![
    Property {
      label: ctx.t("setup.github.access.host"),
      value: found.repo().host().as_str().into(),
      look: Value::Mono,
    },
    Property {
      label: ctx.t("setup.github.access.signedInAs"),
      value: result.and_then(GitHubCheckResult::signed_in_account).map_or_else(
        || ctx.t("setup.github.value.notSignedIn"),
        |login| login.as_str().into(),
      ),
      look: Value::Text(PaletteColor::TextPrimary),
    },
    Property {
      label: ctx.t("setup.github.access.scopes"),
      value: scopes(ctx, result),
      look: Value::Mono,
    },
  ];
  if let Some(expected) = form.expected.get().0 {
    current.push(Property {
      label: ctx.t_args("setup.github.access.projectExpects", [("project", project)]),
      value: expected.as_str().into(),
      look: Value::Text(PaletteColor::TextPrimary),
    });
  }
  current
}

/// The scopes `gh`'s sign-in has, none, or that they are not known.
fn scopes(ctx: &Ctx, result: Option<&GitHubCheckResult>) -> Arc<str> {
  match result {
    Some(GitHubCheckResult::Available { scopes, .. })
      if result.and_then(GitHubCheckResult::signed_in_account).is_some() =>
    {
      if scopes.is_empty() {
        ctx.t("setup.github.access.noScopes")
      } else {
        scopes.join(", ").into()
      }
    }
    _ => ctx.t("setup.github.value.unknown"),
  }
}

/// A titled group of commands (`Section · Use another account` `GXP7z`).
fn section(ctx: &mut Ctx, (title, detail): (&str, &str), advice: &[(&str, GhAdvice)], host: &GitHubHost) -> Element {
  let rows: Vec<Element> = advice
    .iter()
    .map(|(id, advice)| command_row(ctx, id, &advice.command(host)))
    .collect();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t(title)).variant(ProjectsText::SectionTitle.role()))
        .child(Text::new(&ctx.t_args(detail, [("scope", REQUIRED_SCOPE)])).variant(ExtraTypography::Prose.role())),
    )
    .with_children(rows)
    .into()
}

/// What to do with the commands, Close and *Check again* (`Panel actions` `eT0HK`).
fn bottom(ctx: &Ctx, form: &Form, actions: &Actions) -> Row {
  let close = form.access.clone();
  let check = ActionButton::new(
    "github-access-check",
    ButtonKind::Primary,
    ctx.t("setup.github.checkAgain"),
  )
  .icon(Icon::RefreshCw)
  .in_form();
  let check = match form.folder.get().0 {
    Some(folder) if !actions.detect.state().get().is_pending() => {
      let (detect, program) = (actions.clone(), form.program.clone());
      check.on_click(move || detect.detect(folder.clone(), program.get_untracked().0))
    }
    _ => check.disabled(),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("setup.github.access.runHint"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
    )
    .child(
      ActionButton::new(
        "github-access-close",
        ButtonKind::Secondary,
        ctx.t("setup.github.access.close"),
      )
      .icon(Icon::X)
      .in_form()
      .on_click(move || close.set(false)),
    )
    .child(check)
}
