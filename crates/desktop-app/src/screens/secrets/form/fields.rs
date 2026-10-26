//! The fields and actions of New secret and Replace value: New secret is a page under Secrets with the crumb back to
//! it, its fields a group of rows (name, kind, value, project, environment); Replace value is asked in place on the
//! secret's page, its new value in a box with the secret's uses. Each says what was found of the name and why it cannot
//! be saved yet, and a write that failed.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::secret::{Environment, SecretKind, SecretName, SecretNameError};
use orchester_workspace::secrets::{data::ReplacementPermission, form::NameCheck};

use super::{Props, SecretForm};
use crate::{
  screens::{
    project_settings::kit::{Group, back, form_actions, form_row, page_head},
    secrets::{
      state::{EnvironmentChoice, KindChoice},
      words, work,
    },
  },
  theme::{ExtraSpacing, ExtraTypography, SECRETS_PENDING_OPACITY, SettingsText},
  ui::{
    avatar::credential_project_avatar,
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field_error},
    form_section::Label,
    icon::Icon,
    message::message_text,
    notice::{handling_notice, handling_notice_action},
  },
};

/// New secret's page: the crumb back to Secrets, its head, its fields and its actions.
pub(super) fn new_page(ctx: &Ctx, props: &Props, body: Element, footer: Element) -> Element {
  Column::new()
    .id("secret-form")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(back(
      "secret-form-crumb",
      (&ctx.t("project_settings.secrets"), Some(&ctx.t("secrets.form.title"))),
      props.view.list(),
    ))
    .child(page_head(
      "secret-form-head",
      &ctx.t("secrets.form.title"),
      Some(&ctx.t_args(
        "secrets.form.subtitle",
        [("project", props.project.open.project.name().as_str())],
      )),
    ))
    .child(body)
    .child(footer)
    .into()
}

/// Replace value in place: its box on the secret's page (`yTCmk`).
pub(super) fn replace_box(_ctx: &Ctx, body: Element, footer: Element) -> Element {
  Column::new()
    .id("secret-form")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(body)
    .child(footer)
    .into()
}

fn project_row(ctx: &Ctx, props: &Props) -> Row {
  let name = props.project.open.project.name().as_str();
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(credential_project_avatar("secret-project-avatar", name))
    .child(Text::new(name).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&ctx.t("secrets.form.projectNote"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
    )
}

impl SecretForm {
  pub(super) fn fields(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, pending: bool) -> Element {
    let value = self.value_row(ctx, props, check, pending);
    let Some(secret) = &props.secret else {
      let rows = vec![
        self.name_field(ctx, props, check, pending),
        self.kind_field(ctx, pending),
        value,
        form_row(
          Label::new("secret-project-field", ctx.t("secrets.project")),
          project_row(ctx, props),
        ),
        self.environment_field(ctx, pending),
      ];
      return Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(Group::new("secret-form-fields", ctx.t("secrets.form.fields")).rows(rows))
        .child(self.notice(ctx, props, check))
        .into();
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("secrets.rotation.title")).variant(SettingsText::SectionTitle.role()))
      .child(
        Text::new(&ctx.t_args(
          "secrets.rotation.subtitle",
          [
            ("name", secret.name().as_str()),
            ("project", props.project.open.project.name().as_str()),
            ("environment", &ctx.t(words::environment_key(secret.environment()))),
          ],
        ))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
      )
      .child(value)
      .child(self.notice(ctx, props, check))
      .into()
  }

  fn value_row(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, pending: bool) -> Element {
    let replacing = props.secret.is_some();
    let label = ctx.t(if replacing {
      "secrets.rotation.value"
    } else {
      "secrets.form.value"
    });
    let mut value = FieldInput::new("secret-value", label.clone(), self.value.clone()).masked();
    if pending {
      value = value.locked();
    }
    let hint = ctx.t(if replacing {
      "secrets.rotation.hint"
    } else if matches!(check, NameCheck::Taken(_)) {
      "secrets.form.takenValueHint"
    } else {
      "secrets.form.valueHint"
    });
    let field = Column::new()
      .width(Dimension::Pct(100.0))
      .opacity(if pending { SECRETS_PENDING_OPACITY } else { 1.0 })
      .spacing(ExtraSpacing::Snug.role())
      .child(value.render(ctx))
      .child(
        Text::new(&hint)
          .id("secret-value-field-hint")
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      );
    form_row(Label::new("secret-value-field", label), field)
  }

  fn name_field(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, pending: bool) -> Element {
    let hint = match check {
      NameCheck::Invalid(SecretNameError::TooLong { .. }) => {
        Some(ctx.t_args("secrets.form.tooLong", [("max", SecretName::MAX_CHARS.to_string())]))
      }
      NameCheck::Invalid(_) => Some(ctx.t("secrets.form.invalid")),
      NameCheck::Taken(secret) => Some(ctx.t_args(
        "secrets.form.taken",
        [
          ("project", props.project.open.project.name().as_str()),
          ("environment", &ctx.t(words::environment_key(secret.environment()))),
        ],
      )),
      _ => None,
    };
    let mut input = FieldInput::new("secret-name", ctx.t("secrets.form.name"), self.name.clone())
      .mono()
      .placeholder(ctx.t("secrets.form.namePlaceholder"));
    if pending {
      input = input.locked();
    }
    if hint.is_some() {
      input = input.invalid();
    }
    let field = Column::new()
      .width(Dimension::Pct(100.0))
      .opacity(if pending { SECRETS_PENDING_OPACITY } else { 1.0 })
      .spacing(ExtraSpacing::Snug.role())
      .child(input.render(ctx))
      .with_children(hint.map(|hint| {
        Text::new(&hint)
          .id("secret-name-field-hint")
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::Danger)
      }));
    form_row(Label::new("secret-name-field", ctx.t("secrets.form.name")), field)
  }

  fn kind_field(&self, ctx: &mut Ctx, pending: bool) -> Element {
    let label = ctx.t("secrets.form.kind");
    let options = SecretKind::ALL
      .into_iter()
      .map(|kind| SelectOption::new(KindChoice(kind), ctx.t(words::kind_key(kind))))
      .collect();
    let control = if pending {
      FieldInput::new(
        "secret-kind",
        label.clone(),
        lurq::core::Signal::new(ctx.t(words::kind_key(self.kind.get().0)).to_string()),
      )
      .locked()
      .render(ctx)
    } else {
      SelectField::new("secret-kind", label.clone(), self.kind.clone(), options).render(ctx)
    };
    form_row(Label::new("secret-kind-field", label), control)
  }

  fn environment_field(&self, ctx: &mut Ctx, pending: bool) -> Element {
    let label = ctx.t("secrets.environment");
    let options = Environment::ALL
      .into_iter()
      .map(|environment| {
        SelectOption::new(
          EnvironmentChoice(Some(environment)),
          ctx.t(words::environment_key(environment)),
        )
      })
      .collect();
    let control = if pending {
      FieldInput::new(
        "secret-form-environment",
        label.clone(),
        lurq::core::Signal::new(
          ctx
            .t(words::environment_key(
              self.environment.get().0.expect("an environment is chosen"),
            ))
            .to_string(),
        ),
      )
      .locked()
      .render(ctx)
    } else {
      SelectField::new(
        "secret-form-environment",
        label.clone(),
        self.environment.clone(),
        options,
      )
      .render(ctx)
    };
    form_row(Label::new("secret-form-environment-field", label), control)
  }

  fn notice(&self, ctx: &Ctx, props: &Props, check: &NameCheck) -> Element {
    let text = match (&props.secret, check) {
      (Some(_), _) => ctx.t("secrets.rotation.notice"),
      (None, NameCheck::Taken(secret)) => ctx.t_args("secrets.form.takenNotice", [("name", secret.name().as_str())]),
      _ => ctx.t("secrets.form.notice"),
    };
    if props.secret.is_none()
      && let NameCheck::Taken(secret) = check
    {
      let (cancel, view, id) = (self.cancel(props), props.view.clone(), secret.id());
      let environment = secret.environment();
      return handling_notice_action(
        "secret-form-notice",
        "secret-open-existing",
        Icon::ShieldCheck,
        &text,
        &ctx.t("secrets.form.open"),
        move || {
          cancel();
          view.reveal(id, environment);
        },
      )
      .into();
    }
    handling_notice("secret-form-notice", Icon::ShieldCheck, &text).into()
  }

  pub(super) fn footer(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, action: &work::Action) -> Element {
    let state = action.state().get();
    let submit = self.save(ctx, props, check, action, ReplacementPermission::UnusedOnly);
    if self.asking.get()
      && let Some(save) = self.save(ctx, props, check, action, ReplacementPermission::PersonConfirmed)
    {
      return self.question(ctx, save);
    }
    let pending = state.is_pending();
    let replacing = props.secret.is_some();
    let label = match (replacing, pending) {
      (true, true) => "secrets.rotation.replacing",
      (true, false) => "secrets.replace",
      (false, true) => "secrets.form.saving",
      (false, false) => "secrets.form.save",
    };
    let mut save = ActionButton::new("secret-save", ButtonKind::Primary, ctx.t(label))
      .icon(if replacing { Icon::RefreshCw } else { Icon::LockKeyhole })
      .in_form();
    if let Some(submit) = &submit {
      let (asking, submit) = (self.asking.clone(), submit.clone());
      let needs_person = props
        .secret
        .as_ref()
        .is_some_and(|secret| props.listed.replacement_needs_person(secret.id()));
      save = save.on_click(move || {
        if needs_person {
          asking.set(true);
        } else {
          submit();
        }
      });
    }
    if pending {
      save = save.loading();
    }
    let cancel = self.cancel(props);
    let cancel = ActionButton::new("secret-cancel", ButtonKind::Ghost, ctx.t("secrets.cancel"))
      .in_form()
      .on_click(move || cancel());
    let cancel = if pending { cancel.disabled() } else { cancel };
    let reason = (submit.is_none() && !pending).then(|| self.reason(ctx, check, replacing));
    let actions = form_actions(
      "secret-form-actions",
      vec![
        cancel.into(),
        Row::new()
          .opacity(if pending { SECRETS_PENDING_OPACITY } else { 1.0 })
          .child(save)
          .into(),
      ],
      reason.as_deref(),
    );
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(
        state
          .error
          .as_ref()
          .map(|error| field_error("secret-save-error", &message_text(ctx, error))),
      )
      .child(actions)
      .into()
  }

  fn reason(&self, ctx: &Ctx, check: &NameCheck, replacing: bool) -> std::sync::Arc<str> {
    if replacing {
      return ctx.t("secrets.rotation.missing");
    }
    match check {
      NameCheck::Invalid(SecretNameError::TooLong { .. }) => ctx.t("secrets.form.longReason"),
      NameCheck::Invalid(_) => ctx.t("secrets.form.invalidReason"),
      NameCheck::Taken(secret) => ctx.t_args(
        "secrets.form.takenReason",
        [("environment", ctx.t(words::environment_key(secret.environment())))],
      ),
      _ => ctx.t("secrets.form.missing"),
    }
  }
}
