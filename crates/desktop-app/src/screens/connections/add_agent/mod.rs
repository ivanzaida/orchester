//! Add connection for an agent provider (`Connections · Add connection · Claude Code CLI · full page · dark`
//! `cEJy4`, the program and check states of `l2kjVF`): a page in the Connections tab, never a dialog. The
//! person chooses the CLI (the native kinds are not available yet), names the connection, and gets the
//! program found on the search path, which Orchester checks at once (its version, sign-in and models).
//! Only a copy found on the search path is offered: choosing another file is not built in this app.
//! Save connection waits for a check that recognized the CLI; one that is not signed in may be saved and
//! reads Sign-in required. The CLI runs on its own sign-in, on an account, or on an API provider with a key
//! ([`super::provider_form`]), which only a person chooses. The first reason it cannot be saved stays under the
//! actions.

mod check;
mod naming;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureHandle, FutureState},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{AgentProvider, ConnectionAccount, ConnectionCheck},
  name::Name,
  secret::SecretValue,
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    api_provider::{RunsOn, save_cli_in},
    data::{ConnectionDraft, ProjectConnections},
    probe,
  },
  messages::Message,
  tasks::outside::Outside,
};

use self::{
  check::{Checked, check_row, program_row},
  naming::{name_problem, suggest, suggested_name},
};
use super::{
  data::{self, Saved},
  page::{Head, action_bar, page},
  provider_form::{KeyValue, ProviderForm},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsTab, ConnectionsView, ProjectPage, Route, Screen},
  services::{
    Services,
    accounts::{self, Accounts},
  },
  theme::ExtraTypography,
  ui::{
    breadcrumb::Crumb,
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    form_section::{Label, Place, section, stacked_row},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    option_card::OptionCard,
  },
};

#[derive(Clone, PartialEq)]
pub struct AddAgentProps {
  pub project: ShownProject,
}

impl DevtoolsInspectable for AddAgentProps {}

/// A provider as a signal's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kind(AgentProvider);

impl DevtoolsInspectable for Kind {}

/// The CLI's copies on the search path, as the page's future reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Copies(Arc<Vec<PathBuf>>);

impl DevtoolsInspectable for Copies {}

/// The check a page shows, as its future reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Found(Option<ConnectionCheck>);

impl DevtoolsInspectable for Found {}

/// The account a check runs on, as a future's key.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Chosen(ConnectionAccount);

impl DevtoolsInspectable for Chosen {}

type SaveAction = FutureAction<(ConnectionDraft, RunsOn, Option<SecretValue>), Saved, Message>;

pub struct AddAgent {
  save: SaveAction,
  kind: Signal<Kind>,
  name: Signal<String>,
  /// The name the page last suggested: while the field holds it, or nothing, choosing another CLI
  /// suggests another; a name the person typed stays.
  suggested: Signal<String>,
  /// The copy chosen among those found, by its path as shown: the first once they are found, and again whenever
  /// the CLI changes; empty while none is found.
  program: Signal<String>,
  /// Counts Recheck presses: each looks for the program and checks it again.
  recheck: Signal<u64>,
  /// The account chosen, by id; empty while it is the CLI's Default.
  account: Signal<String>,
  /// Whether it runs on an API provider instead, and the provider's fields.
  provider: ProviderForm,
}

impl Component for AddAgent {
  type Props = AddAgentProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = Self {
      save: save_action(ctx),
      kind: ctx.signal(Kind(AgentProvider::CodexCli)),
      name: ctx.signal(String::new()),
      suggested: ctx.signal(String::new()),
      program: ctx.signal(String::new()),
      recheck: ctx.signal(0),
      account: ctx.signal(String::new()),
      provider: ProviderForm::new(ctx),
    };
    let save = page.save.clone();
    let project = ctx.props::<AddAgentProps>().project.open.project.id();
    let navigation = Navigation::of(ctx);
    let changes = Services::of(ctx).changes;
    ctx.watch(&save.state(), move |state| {
      if let Some(Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        let view = ConnectionsView::new(ConnectionsTab::AgentProviders).with_selected(id);
        navigation.replace(Route::Project(project, ProjectPage::Connections(view)));
        // After the form leaves: every screen that shows the project's connections reads them again, the
        // status bar's accounts too.
        changes.record(project, Outside::Connections);
      }
    });
    page
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AddAgentProps>().clone();
    let save = self.save.clone();
    let listed = data::load(ctx, props.project.open.repo_root.clone(), 0);
    let provider = self.kind.get().0;
    let found = found(ctx, provider, self.recheck.get()).state().get();
    self.follow_sources(ctx, &props, provider, &listed, &found);
    let program = self.chosen(found.data.as_ref().map(|copies| copies.0.as_slice()));
    let accounts = accounts::listed(ctx);
    let account = super::account::chosen_account(&accounts, provider, &self.account.get())
      .map_or(ConnectionAccount::Default, ConnectionAccount::of);
    let checked = checked(ctx, (provider, program.clone(), account.clone()), self.recheck.get());
    let state = Checked::of(&found, program.as_ref(), &checked);
    let secrets = listed
      .data
      .as_ref()
      .map(|listed| listed.secrets.clone())
      .unwrap_or_default();
    let reason = self
      .reason(ctx, listed.data.as_ref(), &state, &save.state().get())
      .or_else(|| self.provider.reason(ctx, &secrets));
    let mut sections = vec![
      self.kind_section(ctx, listed.data.as_ref(), props.project.open.project.name().as_str()),
      self.connection_section(ctx, listed.data.as_ref(), (&found, &state), &accounts),
      self.provider.section(ctx, provider, &secrets),
    ];
    if !self.provider.on() {
      sections.push(after(ctx, provider, props.project.open.project.name().as_str()));
    }
    let runs_on = self.provider.runs_on(&secrets, account);
    let bar = self.bar(ctx, &props, (&save, program, &state, runs_on), reason.as_deref());
    let head = head(ctx, &props, &save.state().get());
    page(ctx, Screen::NewConnection, head, sections, bar)
  }
}

impl AddAgent {
  fn follow_sources(
    &self,
    ctx: &Ctx,
    props: &AddAgentProps,
    provider: AgentProvider,
    listed: &FutureState<ProjectConnections, Message>,
    found: &FutureState<Copies, Message>,
  ) {
    if let Some(listed) = listed.fulfilled_data() {
      let project = props.project.open.project.name().as_str();
      let new = suggested_name(ctx.i18n(), listed, provider, project);
      suggest(&self.name, &self.suggested, new);
    }
    if let Some(copies) = found.fulfilled_data() {
      let chosen = self.program.get_untracked();
      if !copies.0.iter().any(|copy| copy.display().to_string() == chosen) {
        let first = copies
          .0
          .first()
          .map_or_else(String::new, |copy| copy.display().to_string());
        if chosen != first {
          self.program.set(first);
        }
      }
    }
  }
  /// The copy to check: the one chosen, else the first found.
  fn chosen(&self, found: Option<&[PathBuf]>) -> Option<PathBuf> {
    let chosen = self.program.get();
    found?.iter().find(|copy| copy.display().to_string() == chosen).cloned()
  }

  /// The first reason Save connection cannot be used, in field order; `None` once it can.
  fn reason(
    &self,
    ctx: &Ctx,
    listed: Option<&ProjectConnections>,
    state: &Checked,
    saving: &FutureState<Saved, Message>,
  ) -> Option<Arc<str>> {
    if let Some(error) = saving.error.as_ref().filter(|_| saving.is_rejected()) {
      return Some(ctx.t_args(
        "connections.dialog.reasons.failed",
        [("reason", &*message_text(ctx, error))],
      ));
    }
    let Some(listed) = listed else {
      return Some(ctx.t("connections.dialog.reasons.reading"));
    };
    if let Some(problem) = name_problem(ctx, listed, &self.name.get()) {
      return Some(ctx.t_args("connections.dialog.reasons.name", [("reason", &*problem)]));
    }
    state.save_reason(ctx)
  }

  /// Cancel and Save connection.
  fn bar(
    &self,
    ctx: &mut Ctx,
    props: &AddAgentProps,
    (save, program, state, runs_on): (&SaveAction, Option<PathBuf>, &Checked, Option<RunsOn>),
    reason: Option<&str>,
  ) -> Element {
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    let cancel =
      ActionButton::new("connection-cancel", ButtonKind::Secondary, ctx.t("connections.cancel")).on_click(move || {
        navigation.go(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        ))
      });
    let button = ActionButton::new("connection-save", ButtonKind::Primary, ctx.t("connections.dialog.save"));
    let draft = match (program, state.check(), Name::new(&self.name.get()), runs_on) {
      (Some(executable), Some(check), Ok(name), Some(runs_on)) if reason.is_none() => Some((
        ConnectionDraft {
          id: None,
          name,
          provider: self.kind.get().0,
          executable,
          check: check.clone(),
        },
        runs_on,
      )),
      _ => None,
    };
    let button = match (save.state().get().is_pending(), draft) {
      (true, _) => button.loading(),
      (false, Some((draft, runs_on))) => {
        let (save, value) = (save.clone(), self.provider.key_value());
        let gate = ctx.use_context::<InputGate>();
        button.on_click(move || saved(&save, (&draft, &runs_on), &value, gate.as_ref()))
      }
      (false, None) => button.disabled(),
    };
    action_bar(vec![cancel.into(), button.into()], reason)
  }

  /// The kinds; choosing another CLI suggests its name, unless the person typed one.
  fn kind_section(&self, ctx: &Ctx, listed: Option<&ProjectConnections>, project: &str) -> Element {
    let card = |provider: AgentProvider, (id, glyph, detail): (&str, Icon, &str)| -> Element {
      let (kind, program) = (self.kind.clone(), self.program.clone());
      let (name, suggested) = (self.name.clone(), self.suggested.clone());
      let suggestion = listed.map(|listed| suggested_name(ctx.i18n(), listed, provider, project));
      let card = OptionCard::new(
        id.to_owned(),
        (glyph, ctx.t(super::words::provider_key(provider))),
        ctx.t(detail),
        self.kind.get().0 == provider,
        move || {
          program.set(String::new());
          kind.set(Kind(provider));
          if let Some(new) = suggestion.clone() {
            suggest(&name, &suggested, new);
          }
        },
      );
      match provider.is_native() {
        true => card.unavailable(ctx.t("connections.kind.nativeUnavailable")).into(),
        false => card.into(),
      }
    };
    let pair = |left: Element, right: Element| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(Column::new().flex(1.0).child(left))
        .child(Column::new().flex(1.0).child(right))
    };
    let cards = Column::new()
      .id("connection-kinds")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(crate::theme::ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Md)
      .child(pair(
        card(
          AgentProvider::CodexCli,
          ("connection-kind-codex", Icon::Terminal, "connections.kind.codexDetail"),
        ),
        card(
          AgentProvider::ClaudeCodeCli,
          (
            "connection-kind-claude",
            Icon::Terminal,
            "connections.kind.claudeDetail",
          ),
        ),
      ))
      .child(pair(
        card(
          AgentProvider::AnthropicApi,
          (
            "connection-kind-anthropic",
            Icon::Cpu,
            "connections.kind.anthropicDetail",
          ),
        ),
        card(
          AgentProvider::OpenAiApi,
          ("connection-kind-openai", Icon::Cpu, "connections.kind.openaiDetail"),
        ),
      ))
      .child(
        Text::new(&ctx.t("connections.kind.nativeHint"))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      );
    section(
      "connection-kind-section",
      &ctx.t("connections.kind.label"),
      None,
      vec![cards.into()],
    )
    .into()
  }

  fn connection_section(
    &self,
    ctx: &mut Ctx,
    listed: Option<&ProjectConnections>,
    (found, state): (&FutureState<Copies, Message>, &Checked),
    accounts: &Accounts,
  ) -> Element {
    let provider = self.kind.get().0;
    let name_error = listed.and_then(|listed| name_problem(ctx, listed, &self.name.get()));
    let mut input = FieldInput::new("connection-name", ctx.t("connections.dialog.name"), self.name.clone());
    if let Some(error) = &name_error {
      input = input.error(error.clone());
    }
    let name = stacked_row(
      Label::new("connection-name", ctx.t("connections.dialog.name")).description(ctx.t("connections.dialog.nameHint")),
      input.render(ctx),
      Place::Within,
    );
    let program = program_row(ctx, provider, (found, &self.program));
    let mut rows = vec![name, program];
    // A connection on an API provider runs on its key, on no account of the CLI.
    if !self.provider.on() {
      rows.push(super::account::add_row(ctx, accounts, provider, &self.account));
    }
    rows.push(check_row(ctx, provider, state, &self.recheck));
    section(
      "connection-section",
      &ctx.t("connections.dialog.connection"),
      None,
      rows,
    )
    .into()
  }
}

/// The CLI's copies found on the search path, newest first, looked for again at each Recheck.
fn found(ctx: &mut Ctx, provider: AgentProvider, recheck: u64) -> FutureHandle<Copies, Message> {
  ctx.future((Kind(provider), recheck), |(Kind(provider), _)| async move {
    background::run(move || Copies(Arc::new(probe::detect_agent(provider)))).await
  })
}

/// The check of `program` as `provider`'s CLI, again at each Recheck; none without a program.
fn checked(
  ctx: &mut Ctx,
  (provider, program, account): (AgentProvider, Option<PathBuf>, ConnectionAccount),
  recheck: u64,
) -> FutureState<Found, Message> {
  let read = ctx.future(
    (Kind(provider), program, Chosen(account), recheck),
    |(Kind(provider), program, Chosen(account), _)| async move {
      let Some(program) = program else {
        return Ok(Found(None));
      };
      background::run(move || Found(probe::check_on(provider, &program, &account))).await
    },
  );
  read.state().get()
}

/// Saves `draft` on `runs_on`, with the key typed for a new secret. Running a connection on an API provider widens
/// what its agents send where, so automation's input saves one only where a person lets it.
fn saved(save: &SaveAction, (draft, runs_on): (&ConnectionDraft, &RunsOn), value: &KeyValue, gate: Option<&InputGate>) {
  if matches!(runs_on, RunsOn::Provider(_)) && !input_may_decide(gate, Guarded::PermissionWidening) {
    tracing::warn!("synthetic input saved a connection on an API provider; not taken");
    return;
  }
  save.run((draft.clone(), runs_on.clone(), value.for_saving(runs_on)));
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let props = ctx.props::<AddAgentProps>().clone();
  let services = Services::of(ctx);
  let (activity, vault) = (services.activity, services.vault);
  ctx.future_action(
    move |(draft, runs_on, value): (ConnectionDraft, RunsOn, Option<SecretValue>)| {
      let (activity, open, vault) = (activity.clone(), props.project.open.clone(), vault.clone());
      async move {
        let spec = operations::connection_save(&open, draft.name.as_str());
        let save = async move {
          background::run(move || save_cli_in(&open.repo_root, vault.get(), draft, runs_on, value).map(Saved)).await?
        };
        activity.track(spec, save).await
      }
    },
  )
}

fn head(ctx: &mut Ctx, props: &AddAgentProps, saving: &FutureState<Saved, Message>) -> Head {
  let navigation = Navigation::of(ctx);
  let project = props.project.open.project.id();
  let status = if saving.is_pending() {
    (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      ctx.t("connections.dialog.saving"),
    )
  } else {
    (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t("connections.dialog.notSaved"),
    )
  };
  Head {
    crumbs: vec![
      Crumb::to(
        ctx.t("connections.title"),
        navigation.to(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        )),
      ),
      Crumb::here(ctx.t("connections.dialog.addTitle")),
    ],
    title: ctx.t("connections.dialog.addTitle"),
    description: ctx.t_args(
      "connections.dialog.description",
      [("project", props.project.open.project.name().as_str())],
    ),
    status,
  }
}

/// How the connection signs in, and what comes after saving.
fn after(ctx: &Ctx, provider: AgentProvider, project: &str) -> Element {
  let program = provider.program_name().unwrap_or_default();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Notice::new(
        "connection-auth",
        NoticeTone::Info,
        ctx.t_args("connections.dialog.authTitle", [("program", program)]),
      )
      .icon(Icon::ShieldCheck)
      .body(ctx.t_args("connections.dialog.authDetail", [("project", project)])),
    )
    .child(
      Text::new(&ctx.t("connections.dialog.nextStep"))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextMuted),
    )
    .into()
}
