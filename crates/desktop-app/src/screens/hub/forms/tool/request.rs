//! What an HTTP or database tool calls (A32 `gLLEv`, A34 `WUuEj`, L20 `Kb6Vk`, L23 `cZGTp`): a request's method, URL,
//! the one host it may call, its authorization, the secret its bearer token is, and its inputs and outputs one per
//! line; a query's credentials secret and the query. A team's secret is typed by name, which each computer fills in; a
//! local project's is picked from the project's secrets of the environment *Access* chooses. A field says why it is
//! wrong under its label as soon as something is typed in it, and an empty one once Create was pressed (L22 `Xp89k`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, SelectOption},
  core::Signal,
};
use orchester_core::{
  id::SecretId,
  resource::{HttpMethod, ToolKind},
  secret::{Environment, Secret},
};
use orchester_workspace::{
  messages::Message,
  tools::form::{ActionText, HttpText, LineError, SecretToolErrors},
};

use super::{
  super::{Form, Line, Look, Need},
  choice::{chosen, segments, text_of},
};
use crate::{
  screens::team_hub::rows::{self, Sub},
  ui::{field::SelectField, icon::Icon, message::message_text},
};

const URL: Line = Line {
  part: "url",
  label: "team_new.tool.url",
  sub: Some("team_new.tool.urlSub"),
  placeholder: Some("team_new.tool.urlPlaceholder"),
  look: Look::Mono,
  required: Need::Always,
};
const HOST: Line = Line {
  part: "host",
  label: "team_new.tool.host",
  sub: Some("team_new.tool.hostSub"),
  placeholder: Some("team_new.tool.hostPlaceholder"),
  look: Look::Mono,
  required: Need::Always,
};
/// Lines of the inputs and outputs: at least 64 high, a few lines (`Inputs` in A32).
const INPUTS: Line = Line {
  part: "inputs",
  label: "team_new.tool.inputs",
  sub: Some("team_new.tool.inputsSub"),
  placeholder: Some("team_new.tool.inputsPlaceholder"),
  look: Look::MonoArea(64.0),
  required: Need::No,
};
const OUTPUTS: Line = Line {
  part: "outputs",
  label: "team_new.tool.outputs",
  sub: Some("team_new.tool.outputsSub"),
  placeholder: Some("team_new.tool.outputsPlaceholder"),
  look: Look::MonoArea(64.0),
  required: Need::No,
};
/// The query: at least 96 high, four lines (`Query` in A34).
const QUERY: Line = Line {
  part: "query",
  label: "team_new.tool.query",
  sub: Some("team_new.tool.querySub"),
  placeholder: Some("team_new.tool.queryPlaceholder"),
  look: Look::MonoArea(96.0),
  required: Need::Always,
};
/// A team secret typed by name: the request's bearer token, or what a query connects with.
const TOKEN_NAMED: Line = Line {
  part: "secret",
  label: "team_new.tool.token",
  sub: Some("team_new.tool.namedSub"),
  placeholder: Some("team_new.tool.tokenPlaceholder"),
  look: Look::Mono,
  required: Need::Always,
};
const CREDENTIALS_NAMED: Line = Line {
  part: "secret",
  label: "team_new.tool.credentials",
  sub: Some("team_new.tool.credentialsSub"),
  placeholder: Some("team_new.tool.credentialsPlaceholder"),
  look: Look::Mono,
  required: Need::Always,
};

const METHODS: [(&str, HttpMethod); 5] = [
  ("get", HttpMethod::Get),
  ("post", HttpMethod::Post),
  ("put", HttpMethod::Put),
  ("patch", HttpMethod::Patch),
  ("delete", HttpMethod::Delete),
];
const NONE: &str = "none";
const BEARER: &str = "bearer";

/// Where the secret comes from: typed by name for the team, or picked from the project's in `environment`.
pub(super) enum Secrets<'a> {
  Named,
  Picked {
    local: LocalSecrets<'a>,
    environment: Option<Environment>,
  },
}

/// A local project's secrets a tool picks from.
#[derive(Clone, Copy, Default)]
pub(in crate::screens) struct LocalSecrets<'a> {
  /// The project's secrets that are not archived; `None` while they are read.
  pub listed: Option<&'a [Secret]>,
  /// Those of them that have no value on this computer, which the secret's row says (L21 `ldRQO`).
  pub valueless: &'a [SecretId],
}

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(super) struct RequestFields {
  method: Signal<String>,
  url: Signal<String>,
  host: Signal<String>,
  auth: Signal<String>,
  inputs: Signal<String>,
  outputs: Signal<String>,
  query: Signal<String>,
  /// The team secret's name as typed.
  named: Signal<String>,
  /// The id of the project secret picked; empty while none is.
  picked: Signal<String>,
}

impl RequestFields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      method: ctx.signal(METHODS[0].0.to_owned()),
      url: ctx.signal(String::new()),
      host: ctx.signal(String::new()),
      auth: ctx.signal(NONE.to_owned()),
      inputs: ctx.signal(String::new()),
      outputs: ctx.signal(String::new()),
      query: ctx.signal(String::new()),
      named: ctx.signal(String::new()),
      picked: ctx.signal(String::new()),
    }
  }

  /// Every field takes the value `action` has; the secret is `named` for the team, `picked` here.
  pub(super) fn load(&self, action: ActionText, (named, picked): (Option<String>, Option<SecretId>)) {
    match action {
      ActionText::Http(http) => {
        self.method.set(text_of(http.method, &METHODS).to_owned());
        self.url.set(http.url);
        self.host.set(http.allowed_host);
        self.auth.set(if http.bearer { BEARER } else { NONE }.to_owned());
        self.inputs.set(http.inputs);
        self.outputs.set(http.outputs);
      }
      ActionText::Database(query) => self.query.set(query),
    }
    self.named.set(named.unwrap_or_default());
    self.picked.set(picked.map(|id| id.to_string()).unwrap_or_default());
  }

  /// What the tool of `kind` (an HTTP request or a database query) calls, as typed.
  pub(super) fn action(&self, kind: ToolKind) -> ActionText {
    if kind == ToolKind::Database {
      return ActionText::Database(self.query.get());
    }
    ActionText::Http(HttpText {
      method: chosen(&self.method, &METHODS),
      url: self.url.get(),
      allowed_host: self.host.get(),
      bearer: self.auth.get() == BEARER,
      inputs: self.inputs.get(),
      outputs: self.outputs.get(),
    })
  }

  /// The team secret's name as typed.
  pub(super) fn named(&self) -> String {
    self.named.get()
  }

  /// The project secret picked, when it is one of `secrets` in `environment`; while the secrets are read (`None`), the
  /// one held.
  pub(super) fn picked(&self, secrets: Option<&[Secret]>, environment: Option<Environment>) -> Option<SecretId> {
    let picked = self.picked.get();
    let Some(secrets) = secrets else {
      return picked.parse().ok();
    };
    secrets
      .iter()
      .find(|secret| secret.id().to_string() == picked && Some(secret.environment()) == environment)
      .map(Secret::id)
  }

  /// The rows of a tool of `kind`, saying `errors`; whether one says why before Create was pressed.
  pub(super) fn rows<B>(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, B>,
    (kind, errors, secrets): (ToolKind, &SecretToolErrors, Secrets<'_>),
  ) -> (Vec<Row>, bool) {
    let said_of = |ctx: &Ctx, value: &Signal<String>, error: &Option<Message>| {
      form.said_of(&value.get(), error.as_ref().map(|error| message_text(ctx, error)))
    };
    if kind == ToolKind::Database {
      let query = said_of(ctx, &self.query, &errors.query);
      let (secret, named) = self.secret_row(ctx, form, (&CREDENTIALS_NAMED, errors, secrets));
      let marked = !form.tried && (query.is_some() || named);
      return (vec![secret, form.line(ctx, &QUERY, &self.query, (query, None))], marked);
    }
    let http = &errors.http;
    let (url, host) = (
      said_of(ctx, &self.url, &http.url),
      said_of(ctx, &self.host, &http.allowed_host),
    );
    let (inputs, outputs) = (lines_said(ctx, &http.inputs), lines_said(ctx, &http.outputs));
    let mut marked = !form.tried && [&url, &host, &inputs, &outputs].iter().any(|said| said.is_some());
    let mut rows = vec![
      self.method_row(ctx, form),
      form.line(ctx, &URL, &self.url, (url, None)),
      form.line(ctx, &HOST, &self.host, (host, None)),
      self.auth_row(ctx, form),
    ];
    if self.auth.get() == BEARER {
      let (secret, named) = self.secret_row(ctx, form, (&TOKEN_NAMED, errors, secrets));
      marked |= !form.tried && named;
      rows.push(secret);
    }
    rows.push(form.line(ctx, &INPUTS, &self.inputs, (inputs, None)));
    rows.push(form.line(ctx, &OUTPUTS, &self.outputs, (outputs, None)));
    (rows, marked)
  }

  fn method_row<B>(&self, ctx: &Ctx, form: &Form<'_, B>) -> Row {
    let options = METHODS.map(|(text, method)| (text, Arc::<str>::from(method.as_str())));
    let value = segments(&form.value_id("method"), &self.method, options, form.locked);
    form.shown("method", (ctx.t("team_new.tool.method"), None), value)
  }

  fn auth_row<B>(&self, ctx: &Ctx, form: &Form<'_, B>) -> Row {
    let options = [
      (NONE, ctx.t("team_new.tool.auth.none")),
      (BEARER, ctx.t("team_new.tool.auth.bearer")),
    ];
    let value = segments(&form.value_id("auth"), &self.auth, options, form.locked);
    form.shown(
      "auth",
      (ctx.t("team_new.tool.auth.label"), Some(ctx.t("team_new.tool.auth.sub"))),
      value,
    )
  }

  /// The secret's row: a name typed for the team (`line`), or a pick of the project's secrets; whether a name typed
  /// says why it cannot be used.
  fn secret_row<B>(
    &self,
    ctx: &mut Ctx,
    form: &Form<'_, B>,
    (line, errors, secrets): (&Line, &SecretToolErrors, Secrets<'_>),
  ) -> (Row, bool) {
    let error = errors.secret.as_ref().map(|error| message_text(ctx, error));
    let Secrets::Picked { local, environment } = secrets else {
      let error = form.said_of(&self.named.get(), error);
      let said = error.is_some();
      return (form.line(ctx, line, &self.named, (error, None)), said);
    };
    let error = form.said(error);
    let options = local
      .listed
      .unwrap_or_default()
      .iter()
      .filter(|secret| Some(secret.environment()) == environment)
      .map(|secret| SelectOption::new(secret.id().to_string(), secret.name().to_string()))
      .collect::<Vec<_>>();
    let placeholder = if options.is_empty() {
      "team_new.tool.noSecrets"
    } else {
      "team_new.tool.chooseSecret"
    };
    let id = format!("{}-{}", form.ids.rows, line.part);
    let mut select = SelectField::new(format!("{id}-input"), ctx.t(line.label), self.picked.clone(), options)
      .placeholder(ctx.t(placeholder))
      .leading(Icon::KeyRound)
      .mono();
    if form.locked {
      select = select.locked();
    }
    let sub = match error {
      Some(words) => {
        select = select.invalid();
        Sub {
          words,
          color: PaletteColor::Danger,
        }
      }
      None
        if self
          .picked(local.listed, environment)
          .is_some_and(|id| local.valueless.contains(&id)) =>
      {
        Sub {
          words: ctx.t("team_new.tool.noValueHere"),
          color: PaletteColor::Warning,
        }
      }
      None => Sub::quiet(ctx.t("team_new.tool.pickedSub")),
    };
    let label = form.label(ctx, line.label, line.required);
    (rows::row(&id, &label, Some(sub), select.render(ctx)), false)
  }
}

/// What the wrong lines of the inputs or the outputs say, one after the other: *Line 1: … · line 2: …* (L22 `Xp89k`).
fn lines_said(ctx: &Ctx, errors: &[LineError]) -> Option<Arc<str>> {
  let mut errors = errors.iter();
  let first = errors.next()?;
  let first = ctx.t_args(
    "team_new.tool.lineFirst",
    [
      ("line", first.line.to_string()),
      ("error", message_text(ctx, &first.reason).to_string()),
    ],
  );
  Some(errors.fold(first, |before, next| {
    ctx.t_args(
      "team_new.tool.lineNext",
      [
        ("before", before.to_string()),
        ("line", next.line.to_string()),
        ("error", message_text(ctx, &next.reason).to_string()),
      ],
    )
  }))
}
