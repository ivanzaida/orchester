//! Change GitHub account or access (01.D `hNPA1`), opened from a GitHub connection: the account
//! `gh` is signed in to, the account the project expects, and the `gh` commands that sign in,
//! switch accounts or add the `repo` scope, each with Copy. Orchester never runs them; the user
//! runs them in a terminal and checks again.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{GhAdvice, GitHubCheckResult, GitHubConnection, GitHubLogin, REQUIRED_SCOPE};

use super::github_status::command_ids;
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::{ActionButton, SKIPPED_BY_TAB},
    command_row::{CommandPlace, Copied, command_row},
    form::form,
    icon::Icon,
    property::property,
  },
};

/// Panel width (`hNPA1`), divider thickness (`EdRpj`) and button height (`G7cQlZ`).
const PANEL_WIDTH: f32 = 480.0;
const DIVIDER: f32 = 1.0;
const BUTTON_HEIGHT: f32 = 36.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct AccountPanelProps {
  pub connection: GitHubConnection,
  /// The login the project's repository link expects, if it is linked through this connection.
  pub expected: Option<GitHubLogin>,
  pub project: Arc<str>,
  /// `None` while a check runs.
  pub check: Option<Handler>,
  pub close: Handler,
}

impl PartialEq for AccountPanelProps {
  fn eq(&self, other: &Self) -> bool {
    self.connection == other.connection
      && self.expected == other.expected
      && self.project == other.project
      && self.check.is_some() == other.check.is_some()
  }
}

impl lurq::app::component::DevtoolsInspectable for AccountPanelProps {}

pub struct AccountPanel {
  copied: Signal<Copied>,
  /// Focused when the panel opens, so that Tab starts at its first control.
  frame: ElementRef,
}

impl Component for AccountPanel {
  type Props = AccountPanelProps;

  fn create(ctx: &mut Ctx) -> Self {
    let panel = Self {
      copied: ctx.signal(Copied::default()),
      frame: ElementRef::new(),
    };
    ctx.focus(&panel.frame);
    panel
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AccountPanelProps>().clone();
    let panel = Column::new()
      .id("github-account-panel")
      .ref_element(self.frame.clone())
      .focusable(true)
      .tab_index(SKIPPED_BY_TAB)
      .width(Dimension::Px(PANEL_WIDTH))
      .padding(ExtraSpacing::Step.role())
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(heading(ctx))
      .child(current(ctx, &props))
      .child(self.section(
        ctx,
        "github.account.otherTitle",
        "github.account.otherDetail",
        &props,
        &[GhAdvice::Login, GhAdvice::Switch(props.expected.clone())],
      ))
      .child(self.section(
        ctx,
        "github.account.accessTitle",
        "github.account.accessDetail",
        &props,
        &[GhAdvice::Refresh],
      ))
      .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER)).background(ExtraColor::Hairline.role()))
      .child(actions(ctx, &props));
    backdrop(form(props.check.clone(), panel))
  }
}

impl AccountPanel {
  /// A titled group of command rows (`SbX5F`, `VCmRM`).
  fn section(&self, ctx: &Ctx, title: &str, detail: &str, props: &AccountPanelProps, advice: &[GhAdvice]) -> Element {
    let rows = advice.iter().map(|advice| {
      let (id, copy_id) = command_ids(advice);
      let command = advice.command(props.connection.host());
      command_row(ctx, id, copy_id, &command, CommandPlace::Dialog, &self.copied)
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t(title)).variant(ExtraTypography::NoticeHeading.role()))
      .child(Text::new(&ctx.t_args(detail, [("scope", REQUIRED_SCOPE)])).variant(ExtraTypography::Detail.role()))
      .with_children(rows)
      .into()
  }
}

/// Title and the sign-in rule (`SfNZ8`).
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(Text::new(&ctx.t("github.account.title")).variant(TypographyStyle::Label))
    .child(Text::new(&ctx.t("github.account.detail")).variant(ExtraTypography::Detail.role()))
    .into()
}

/// Host, the signed-in account, its scopes and the account the project expects (`XeBzd`).
fn current(ctx: &Ctx, props: &AccountPanelProps) -> Element {
  let connection = &props.connection;
  let result = connection.last_check().map(|check| &check.result);
  let account = result
    .and_then(|result| result.signed_in_account())
    .map_or_else(|| ctx.t("github.connection.notSignedIn"), |login| login.as_str().into());
  let scopes = match result {
    Some(GitHubCheckResult::Available { scopes, .. }) if account_known(result) && !scopes.is_empty() => {
      scopes.join(", ").into()
    }
    _ if account_known(result) => ctx.t("github.connection.noScopes"),
    _ => ctx.t("github.connection.unknown"),
  };
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .child(property(&ctx.t("github.connection.host"), connection.host().as_str()))
    .child(property(&ctx.t("github.account.signedInAs"), &account))
    .child(property(&ctx.t("github.connection.scopes"), &scopes));
  if let Some(expected) = &props.expected {
    column = column.child(property(
      &ctx.t_args("github.account.projectExpects", [("project", &*props.project)]),
      expected.as_str(),
    ));
  }
  column.into()
}

fn account_known(result: Option<&GitHubCheckResult>) -> bool {
  result.is_some_and(|result| result.signed_in_account().is_some())
}

/// The hint, Close and Check again (`dLLHu`).
fn actions(ctx: &Ctx, props: &AccountPanelProps) -> Element {
  let close = props.close.clone();
  let check = ActionButton::new(ctx.t("github.checkAgain"))
    .primary()
    .icon(Icon::RefreshCw)
    .id("github-account-check")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let check = match props.check.clone() {
    Some(handler) => check.on_click(move || handler()),
    // Waits for the running check.
    None => check.on_click(|| {}).enabled(false),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("github.account.runHint"))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    )
    .child(
      ActionButton::new(ctx.t("common.close"))
        .icon(Icon::X)
        .id("github-account-close")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || close())
        .render(ctx),
    )
    .child(check.render(ctx))
    .into()
}
