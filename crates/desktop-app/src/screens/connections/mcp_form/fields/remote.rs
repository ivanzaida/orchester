//! The remote server's section: its endpoint and its authorization. The Authorization select chooses the scheme;
//! Basic asks for a user name and a custom header for its name, and every scheme but none for the project secret it
//! sends (the token, the password, the pasted credential or the header's value), named after what it is. No design
//! node has the scheme's fields yet: they follow the section's own rows (a stacked label over the control, its hint
//! under the label, an error under the control).

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::SpacingSize},
  components::{Column, SelectOption},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::SecretId, secret::Secret};
use orchester_workspace::connections::mcp::{AuthorizationChoice, AuthorizationText};

use super::{Fields, Shown, input};
use crate::ui::{
  field::{SelectField, field_error},
  form_section::{Label, Place, section, stacked_row},
};

/// The Authorization select's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Scheme(AuthorizationChoice);

impl DevtoolsInspectable for Scheme {}

/// The secret the authorization sends, if one is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chosen(Option<SecretId>);

impl DevtoolsInspectable for Chosen {}

/// The authorization's fields as signals.
#[derive(Clone)]
pub(super) struct AuthorizationFields {
  scheme: Signal<Scheme>,
  secret: Signal<Chosen>,
  username: Signal<String>,
  header: Signal<String>,
}

impl AuthorizationFields {
  pub(super) fn new(ctx: &mut Ctx, text: &AuthorizationText) -> Self {
    Self {
      scheme: ctx.signal(Scheme(text.choice)),
      secret: ctx.signal(Chosen(text.secret)),
      username: ctx.signal(text.username.clone()),
      header: ctx.signal(text.header.clone()),
    }
  }

  pub(super) fn text(&self) -> AuthorizationText {
    AuthorizationText {
      choice: self.scheme.get().0,
      secret: self.secret.get().0,
      username: self.username.get(),
      header: self.header.get(),
    }
  }

  pub(super) const fn username(&self) -> &Signal<String> {
    &self.username
  }

  pub(super) const fn header(&self) -> &Signal<String> {
    &self.header
  }

  /// Whether the secret's error shows: once a save was `tried`, or while a secret is chosen (one that is gone).
  pub(super) fn secret_shown(&self, tried: bool) -> bool {
    tried || self.secret.get_untracked().0.is_some()
  }
}

/// The keys of the secret's label and hint for `choice`.
const fn secret_words(choice: AuthorizationChoice) -> (&'static str, &'static str) {
  match choice {
    AuthorizationChoice::None | AuthorizationChoice::Bearer => (
      "connections.mcp.dialog.secret.bearer",
      "connections.mcp.dialog.secretHint.bearer",
    ),
    AuthorizationChoice::Basic => (
      "connections.mcp.dialog.secret.basic",
      "connections.mcp.dialog.secretHint.basic",
    ),
    AuthorizationChoice::BasicCredential => (
      "connections.mcp.dialog.secret.basicCredential",
      "connections.mcp.dialog.secretHint.basicCredential",
    ),
    AuthorizationChoice::Header => (
      "connections.mcp.dialog.secret.header",
      "connections.mcp.dialog.secretHint.header",
    ),
  }
}

impl Fields {
  pub(super) fn remote_section(&self, ctx: &mut Ctx, errors: &Shown, secrets: &[Secret]) -> Element {
    let url = input(
      ctx,
      ("connection-mcp-url", "connections.mcp.property.endpoint"),
      &self.url,
      errors.url.clone(),
    )
    .placeholder(ctx.t("connections.mcp.dialog.urlPlaceholder"))
    .mono()
    .render(ctx);
    let choice = self.authorization.scheme.get().0;
    let mut rows = vec![
      stacked_row(
        Label::new("connection-mcp-url", ctx.t("connections.mcp.property.endpoint")),
        url,
        Place::Within,
      ),
      self.scheme_row(ctx, choice),
    ];
    if choice == AuthorizationChoice::Basic {
      rows.push(text_row(
        ctx,
        ("connection-mcp-username", "connections.mcp.dialog.username"),
        (&self.authorization.username, errors.username.clone()),
        "connections.mcp.dialog.usernameHint",
      ));
    }
    if choice == AuthorizationChoice::Header {
      rows.push(text_row(
        ctx,
        ("connection-mcp-header", "connections.mcp.dialog.header"),
        (&self.authorization.header, errors.header.clone()),
        "connections.mcp.dialog.headerHint",
      ));
    }
    if choice != AuthorizationChoice::None {
      rows.push(self.secret_row(ctx, choice, errors.secret.as_deref(), secrets));
    }
    section(
      "connection-mcp-remote-section",
      &ctx.t("connections.mcp.typeHttp"),
      None,
      rows,
    )
    .into()
  }

  fn scheme_row(&self, ctx: &Ctx, choice: AuthorizationChoice) -> Element {
    let options = [
      (AuthorizationChoice::None, "connections.mcp.authorization.none"),
      (AuthorizationChoice::Bearer, "connections.mcp.authorization.bearer"),
      (AuthorizationChoice::Basic, "connections.mcp.authorization.basic"),
      (
        AuthorizationChoice::BasicCredential,
        "connections.mcp.authorization.basicCredential",
      ),
      (AuthorizationChoice::Header, "connections.mcp.authorization.header"),
    ]
    .into_iter()
    .map(|(choice, key)| SelectOption::new(Scheme(choice), ctx.t(key).to_string()))
    .collect();
    let select = SelectField::new(
      "connection-mcp-authorization",
      ctx.t("connections.property.authentication"),
      self.authorization.scheme.clone(),
      options,
    )
    .render(ctx);
    let place = if choice == AuthorizationChoice::None {
      Place::Last
    } else {
      Place::Within
    };
    stacked_row(
      Label::new(
        "connection-mcp-authorization",
        ctx.t("connections.property.authentication"),
      )
      .description(ctx.t("connections.mcp.dialog.authorizationHint")),
      select,
      place,
    )
  }

  /// The project secret the scheme sends; a secret the project no longer has stays chosen, its error under it.
  fn secret_row(&self, ctx: &Ctx, choice: AuthorizationChoice, error: Option<&str>, secrets: &[Secret]) -> Element {
    let (label, hint) = secret_words(choice);
    let options = secrets
      .iter()
      .map(|secret| {
        let name = ctx.t_args(
          "connections.mcp.secretName",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        );
        SelectOption::new(Chosen(Some(secret.id())), name.to_string())
      })
      .collect();
    let placeholder = if secrets.is_empty() {
      "connections.mcp.dialog.noSecrets"
    } else {
      "connections.mcp.dialog.chooseSecret"
    };
    let mut select = SelectField::new(
      "connection-mcp-secret",
      ctx.t(label),
      self.authorization.secret.clone(),
      options,
    )
    .placeholder(ctx.t(placeholder));
    if error.is_some() {
      select = select.invalid();
    }
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(select.render(ctx))
      .with_children(error.map(|error| field_error("connection-mcp-secret-error", error)));
    stacked_row(
      Label::new("connection-mcp-secret", ctx.t(label)).description(ctx.t(hint)),
      control,
      Place::Last,
    )
  }
}

/// A text field of the authorization, named by `key`, with `hint` under its label.
fn text_row(
  ctx: &mut Ctx,
  (id, key): (&str, &str),
  (value, error): (&Signal<String>, Option<std::sync::Arc<str>>),
  hint: &str,
) -> Element {
  let field = input(ctx, (id, key), value, error).mono().render(ctx);
  stacked_row(
    Label::new(id, ctx.t(key)).description(ctx.t(hint)),
    field,
    Place::Within,
  )
}
