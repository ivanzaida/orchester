//! Fields of the GitHub connection dialog (`iKxHm` in 04.06): name and type, host and program,
//! the program file, how it signs in, what the check found and what the project gets.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{GH_PROGRAM, GitHubCheckResult, GitHubHost, GitHubState, REQUIRED_SCOPE};

use super::{
  DialogMode,
  dialog::PickAction,
  form::{connection_name, pair, select_field, type_field},
  github_dialog::{GhProgram, GhVerification, GitHubDialog, HostChoice},
  status::CHECKING,
};
use crate::{
  display,
  screens::github::status::status_label,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{FieldNote, ValueTone, field, text_input, value_box},
    icon::Icon,
    message_text,
    notice::Notice,
    status_label::status_label as label,
  },
};

/// Field and Check again heights (`ApOxr`, `V9l42`), and the longest program path shown in full.
const FIELD_HEIGHT: f32 = 40.0;
const RECHECK_HEIGHT: f32 = 30.0;
const PATH_CHARS: usize = 44;

impl GitHubDialog {
  pub(super) fn form(&self, ctx: &Ctx, program: &GhProgram, pick: &PickAction) -> Element {
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Form.role())
      .child(pair(self.name_field(ctx), type_field(ctx, &self.kind, self.editing())))
      .child(pair(self.host_field(ctx), program_field(ctx, program)));
    if self.host.get() == HostChoice::Other && !self.editing() {
      column = column.child(self.other_host_field(ctx, program));
    }
    column
      .child(self.program_file_field(ctx, program, pick))
      .child(Notice::new(
        Icon::Terminal,
        ctx.t("github.dialog.authTitle"),
        ctx.t("github.dialog.authDetail"),
      ))
      .child(self.verification_row(ctx, program))
      .child(Text::new(self.context.project.project.name().as_str()).variant(ExtraTypography::CardTitle.role()))
      .child(Text::new(&self.next_step(ctx)).variant(ExtraTypography::Explanation.role()))
      .into()
  }

  /// What saving means for the project (`z6Awn`): with its repository linked on this host, the
  /// account it expects and the repository pull request steps use.
  fn next_step(&self, ctx: &Ctx) -> Arc<str> {
    let project = self.context.project.project.name().as_str();
    let host = self.chosen_host().ok();
    match self
      .context
      .link
      .as_deref()
      .filter(|link| Some(link.repo().host()) == host.as_ref())
    {
      Some(link) => {
        let repository = format!("{}/{}", link.repo().owner(), link.repo().name());
        ctx.t_args(
          "github.dialog.nextStepLinked",
          [
            ("project", project),
            ("account", link.expected_login().as_str()),
            ("repository", repository.as_str()),
            ("remote", link.remote().as_str()),
          ],
        )
      }
      None => ctx.t_args("github.dialog.nextStep", [("project", project)]),
    }
  }

  fn name_field(&self, ctx: &Ctx) -> Element {
    let invalid = connection_name(&self.name.get(), &self.context.taken).err();
    let input = text_input(
      ctx,
      "connection-name",
      self.name.clone(),
      invalid.is_some(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(FIELD_HEIGHT));
    let note = invalid.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, &error)));
    field(&ctx.t("connections.dialog.name"), input.into(), note)
  }

  /// github.com or another host; fixed while editing, since the host is what the connection is.
  fn host_field(&self, ctx: &Ctx) -> Element {
    let label = ctx.t("github.connection.host");
    if let DialogMode::EditGitHub(connection) = &self.context.mode {
      let control = value_box(ctx, "github-host", connection.host().as_str(), ValueTone::Value, None);
      return field(&label, control, FieldNote::None);
    }
    let options = [
      (HostChoice::GitHubCom, Arc::<str>::from(GitHubHost::GITHUB_COM)),
      (HostChoice::Other, ctx.t("github.dialog.otherHost")),
    ];
    select_field(ctx, "github-host", &label, self.host.clone(), options)
  }

  /// The GitHub Enterprise host, e.g. github.example.com.
  fn other_host_field(&self, ctx: &Ctx, program: &GhProgram) -> Element {
    let error = match (&program.host, self.other_host.get().trim().is_empty()) {
      (Err(error), false) => Some(error.clone()),
      _ => None,
    };
    let input = text_input(
      ctx,
      "github-other-host",
      self.other_host.clone(),
      error.is_some(),
      TypographyStyle::FieldLabel,
    )
    .placeholder(&ctx.t("github.dialog.otherHostPlaceholder"))
    .height(Dimension::Px(FIELD_HEIGHT));
    let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, &error)));
    field(&ctx.t("github.dialog.hostName"), input.into(), note)
  }

  /// The program file: found on the `PATH`, or chosen with the file picker (`rLtOO`).
  fn program_file_field(&self, ctx: &Ctx, program: &GhProgram, pick: &PickAction) -> Element {
    let pick = pick.clone();
    let title = ctx.t("github.dialog.chooseProgram");
    let choose: Box<dyn Fn() + Send + Sync> = Box::new(move || pick.run(title.clone()));
    let (text, tone) = match &program.executable {
      Some(path) => (display::shorten_path(path, PATH_CHARS), ValueTone::Value),
      None => (
        ctx.t("connections.dialog.noProgram").to_string(),
        ValueTone::Placeholder,
      ),
    };
    let control = value_box(ctx, "connection-program", &text, tone, Some((Icon::FolderOpen, choose)));
    let hint = match (program.chosen, program.found) {
      (true, _) => ctx.t("connections.dialog.programChosen"),
      (false, 0) => ctx.t_args("connections.dialog.programMissing", [("program", GH_PROGRAM)]),
      (false, 1) => ctx.t("connections.dialog.programFoundOne"),
      (false, count) => ctx.t_args(
        "connections.dialog.programFoundMany",
        [("count", count.to_string().as_str())],
      ),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(
        &ctx.t("connections.property.executable"),
        control,
        FieldNote::None,
      ))
      .child(Text::new(&hint).variant(ExtraTypography::Footnote.role()))
      .into()
  }

  /// The check's outcome and Check again (`SDBHk`).
  fn verification_row(&self, ctx: &Ctx, program: &GhProgram) -> Element {
    let (glyph, status, detail) = match &program.verification {
      GhVerification::Detecting => (CHECKING.0, ctx.t(CHECKING.1), ctx.t("connections.dialog.detecting")),
      GhVerification::Checking => (CHECKING.0, ctx.t(CHECKING.1), ctx.t("github.dialog.checking")),
      GhVerification::NoHost => (
        Icon::Circle,
        ctx.t("github.status.notChecked"),
        ctx.t("github.dialog.enterHost"),
      ),
      GhVerification::NotFound => (
        Icon::AlertCircle,
        ctx.t("github.status.programMissing"),
        ctx.t("connections.dialog.chooseToCheck"),
      ),
      GhVerification::Failed(error) => (
        Icon::AlertCircle,
        ctx.t("github.status.couldNotRun"),
        message_text(ctx, error),
      ),
      GhVerification::Done(check) => {
        let state = GitHubState::of(Some(&check.result), None);
        let (glyph, key) = status_label(&state);
        (
          glyph,
          ctx.t_args(key, [("scope", REQUIRED_SCOPE)]),
          check_detail(ctx, &check.result),
        )
      }
    };
    let recheck = ActionButton::new(ctx.t("github.checkAgain"))
      .icon(Icon::RefreshCw)
      .id("connection-recheck")
      .in_form()
      .height(Dimension::Px(RECHECK_HEIGHT));
    let recheck = match program.verification {
      GhVerification::Done(_) | GhVerification::Failed(_) => {
        let attempt = self.attempt.clone();
        recheck.on_click(move || attempt.update(|attempt| *attempt += 1))
      }
      // Nothing to check again yet: the check runs, or there is nothing to check.
      GhVerification::Detecting | GhVerification::Checking | GhVerification::NotFound | GhVerification::NoHost => {
        recheck.on_click(|| {}).enabled(false)
      }
    };
    Row::new()
      .id("connection-verification")
      .width(Dimension::Pct(100.0))
      // Check again is as tall as the row; its focus ring reaches into the form's spacing.
      .overflow_visible()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(label(glyph, &status))
      .child(Text::new(&detail).variant(TypographyStyle::Caption).flex(1.0))
      .child(recheck.render(ctx))
      .into()
  }
}

/// The program as the check found it (`CkIyc`): "gh 2.62.0 · found on PATH".
fn program_field(ctx: &Ctx, program: &GhProgram) -> Element {
  let (text, tone) = match &program.verification {
    GhVerification::Done(check) => match (&check.result, program.chosen) {
      (GitHubCheckResult::Available { version, .. }, false) => (
        ctx.t_args("github.dialog.programOnPath", [("version", version.as_str())]),
        ValueTone::Value,
      ),
      (GitHubCheckResult::Available { version, .. }, true) => (
        ctx.t_args("github.dialog.programChosen", [("version", version.as_str())]),
        ValueTone::Value,
      ),
      _ => (ctx.t("github.dialog.programNotGh"), ValueTone::Invalid),
    },
    GhVerification::NotFound => (ctx.t("github.dialog.programNotOnPath"), ValueTone::Placeholder),
    GhVerification::Detecting | GhVerification::Checking => (ctx.t(CHECKING.1), ValueTone::Placeholder),
    GhVerification::NoHost | GhVerification::Failed(_) => (ctx.t("connections.notCheckedYet"), ValueTone::Placeholder),
  };
  let control = value_box(ctx, "github-program", &text, tone, None);
  field(&ctx.t("connections.property.program"), control, FieldNote::None)
}

/// What a finished check found (`sPfNM`): the account and its scopes, or why it cannot be used.
fn check_detail(ctx: &Ctx, result: &GitHubCheckResult) -> Arc<str> {
  let GitHubCheckResult::Available { version, scopes, .. } = result else {
    let key = match result {
      GitHubCheckResult::ExecutableMissing => "github.hint.programMissing",
      GitHubCheckResult::NotRecognized => "github.hint.unrecognized",
      _ => "github.hint.couldNotRun",
    };
    return ctx.t(key);
  };
  match result.signed_in_account() {
    Some(account) if scopes.is_empty() => ctx.t_args("github.dialog.signedInNoScopes", [("account", account.as_str())]),
    Some(account) => ctx.t_args(
      "github.dialog.signedIn",
      [("account", account.as_str()), ("scopes", scopes.join(", ").as_str())],
    ),
    None => ctx.t_args("github.dialog.notSignedIn", [("version", version.as_str())]),
  }
}
