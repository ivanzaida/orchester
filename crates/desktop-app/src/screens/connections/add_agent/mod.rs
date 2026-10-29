//! Add provider (`10 · Add provider · own sign-in` `LaTB4`, `10b · on an API provider` `hc1vS`): a page in the
//! Connections tab, never a dialog. The person chooses the kind, names the connection, and for a CLI gets the program
//! found on the search path, which Orchester checks at once (its version, sign-in and models); the check says what it
//! found under the program. Only a copy found on the search path is offered: choosing another file is not built in
//! this app. Save connection waits for a check that recognized the CLI; one that is not signed in may be saved and
//! reads Sign-in required. The CLI runs on its own sign-in, on an account, or on an API provider with a key
//! ([`super::provider_form`]), which only a person chooses. The Anthropic API's native kind has its own fields instead
//! ([`native`]); the OpenAI API's is locked until the runner runs its agents. The first reason it cannot be saved
//! stays before the actions, or what comes after saving once it can be.

mod check;
mod kinds;
pub(super) mod naming;
mod native;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureHandle, FutureState},
  },
  core::Signal,
  node::Element,
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
  check::{Checked, program_row},
  naming::{name_problem, suggest, suggested_name},
};
use super::{
  data::{self, Saved},
  native_form::NativeForm,
  page::{Head, action_bar_noted, page},
  provider_form::ProviderForm,
};
use crate::{
  mcp::input_gate::InputGate,
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route, Screen},
  screens::project_settings::kit::{FormField, Tone, form_group},
  services::{
    Services,
    accounts::{self, Accounts},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    message::message_text,
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
  /// Saves a native connection with the key typed for a new secret, if any.
  save_native: native::SaveNative,
  kind: Signal<Kind>,
  name: Signal<String>,
  /// The name the page last suggested: while the field holds it, or nothing, choosing another CLI
  /// suggests another; a name the person typed stays.
  suggested: Signal<String>,
  /// The copy chosen among those found, by its path as shown: the first once they are found, and again whenever
  /// the CLI changes; empty while none is found.
  program: Signal<String>,
  /// Counts Check presses: each looks for the program and checks it again.
  recheck: Signal<u64>,
  /// The account chosen, by id; empty while it is the CLI's Default.
  account: Signal<String>,
  /// Whether it runs on an API provider instead, and the provider's fields.
  provider: ProviderForm,
  /// A native connection's fields; the Anthropic API is the native kind this version runs.
  native: NativeForm,
}

impl Component for AddAgent {
  type Props = AddAgentProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = Self {
      save: save_action(ctx),
      save_native: native::save_action(ctx),
      kind: ctx.signal(Kind(AgentProvider::CodexCli)),
      name: ctx.signal(String::new()),
      suggested: ctx.signal(String::new()),
      program: ctx.signal(String::new()),
      recheck: ctx.signal(0),
      account: ctx.signal(String::new()),
      provider: ProviderForm::new(ctx),
      native: NativeForm::new(ctx, AgentProvider::AnthropicApi),
    };
    let project = ctx.props::<AddAgentProps>().project.open.project.id();
    let navigation = Navigation::of(ctx);
    let changes = Services::of(ctx).changes;
    let leave = move |id| {
      navigation.replace(Route::Project(
        project,
        ProjectPage::Connections(ConnectionsView::of(id)),
      ));
      // After the form leaves: every screen that shows the project's connections reads them again, the status bar's
      // accounts too.
      changes.record(project, Outside::Connections);
    };
    let leave_native = leave.clone();
    ctx.watch(&page.save.state(), move |state| {
      if let Some(Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        leave(id);
      }
    });
    ctx.watch(&page.save_native.state(), move |state| {
      if let Some(Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        leave_native(id);
      }
    });
    page
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AddAgentProps>().clone();
    let save = self.save.clone();
    let listed = data::load(ctx, props.project.open.repo_root.clone(), 0);
    let provider = self.kind.get().0;
    if provider.is_native() {
      return self.native_page(ctx, &props, &listed);
    }
    let found = found(ctx, provider, self.recheck.get()).state().get();
    self.follow_sources(ctx, &props, provider, &listed, &found);
    let program = self.chosen(found.data.as_ref().map(|copies| copies.0.as_slice()));
    let accounts = accounts::listed(ctx);
    let account = super::account::chosen_account(&accounts, provider, &self.account.get())
      .map_or(ConnectionAccount::Default, ConnectionAccount::of);
    let checked = checked(ctx, (provider, program.clone(), account.clone()), self.recheck.get());
    let state = Checked::of(&found, program.as_ref(), &checked);
    let known = listed.data.clone().unwrap_or_default();
    let reason = self
      .reason(ctx, listed.data.as_ref(), &state, &save.state().get())
      .or_else(|| self.provider.reason(ctx, &known));
    let mut sections = vec![
      self.kind_section(ctx, listed.data.as_ref(), props.project.open.project.name().as_str()),
      self.connection_section(ctx, (listed.data.as_ref(), &known), (&found, &state), &accounts),
    ];
    sections.extend(self.provider.warning(ctx));
    let runs_on = self.provider.runs_on(&known, account);
    let bar = self.bar(ctx, &props, (&save, program, &state, runs_on), reason.as_deref());
    let head = head(ctx, &props, self.provider.on());
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

  /// Cancel and Save connection, after why Save cannot be used or, once it can, what comes after saving.
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
      ActionButton::new("connection-cancel", ButtonKind::Ghost, ctx.t("connections.cancel")).on_click(move || {
        navigation.go(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        ))
      });
    let button =
      ActionButton::new("connection-save", ButtonKind::Primary, ctx.t("connections.dialog.save")).icon(Icon::Check);
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
        let (save, guard) = (save.clone(), self.provider.save_guard());
        let gate = ctx.use_context::<InputGate>();
        button.on_click(move || {
          if let Some(value) = guard.admit(&runs_on, gate.as_ref()) {
            save.run((draft.clone(), runs_on.clone(), value));
          }
        })
      }
      (false, None) => button.disabled(),
    };
    let refusal = self.provider.refusal(ctx);
    let next = match self.provider.on() {
      true => ctx.t("connections.apiProvider.nextStep"),
      false => ctx.t("connections.dialog.nextStep"),
    };
    let note = match refusal.as_deref().or(reason) {
      Some(reason) => (reason.to_owned(), Tone::Muted),
      None => (next.to_string(), Tone::Success),
    };
    action_bar_noted(vec![cancel.into(), button.into()], Some((note.0.as_str(), note.1)))
  }

  /// The Connection group (`Connection` `H3e3OD`): the name, the program with its check, what it runs on, and the
  /// account on its own sign-in or the provider's fields.
  fn connection_section(
    &self,
    ctx: &mut Ctx,
    (listed, known): (Option<&ProjectConnections>, &ProjectConnections),
    (found, state): (&FutureState<Copies, Message>, &Checked),
    accounts: &Accounts,
  ) -> Element {
    let provider = self.kind.get().0;
    let name = self.name_row(ctx, listed);
    let program = program_row(
      ctx,
      (provider, self.provider.on()),
      (found, &self.program),
      (state, &self.recheck),
    );
    let mut rows = vec![name, program];
    let mut runs_on = self.provider.rows(ctx, provider, known);
    // A connection on an API provider runs on its key, on no account of the CLI: the account follows *Runs on*.
    if !self.provider.on() {
      runs_on.push(super::account::add_row(ctx, accounts, provider, &self.account));
    }
    rows.extend(runs_on);
    form_group(
      "connection-section",
      &ctx.t("connections.dialog.connection"),
      None,
      rows,
    )
  }

  /// The connection's name, with why it cannot be the connection's.
  fn name_row(&self, ctx: &mut Ctx, listed: Option<&ProjectConnections>) -> Element {
    let name_error = listed.and_then(|listed| name_problem(ctx, listed, &self.name.get()));
    let mut input = FieldInput::new("connection-name", ctx.t("connections.dialog.name"), self.name.clone());
    if let Some(error) = &name_error {
      input = input.error(error.clone());
    }
    let input = input.render(ctx);
    FormField::new("connection-name", ctx.t("connections.dialog.name"))
      .hint(ctx.t("connections.dialog.nameHint"))
      .render(ctx, input)
  }
}

/// The CLI's copies found on the search path, newest first, looked for again at each Check.
fn found(ctx: &mut Ctx, provider: AgentProvider, recheck: u64) -> FutureHandle<Copies, Message> {
  ctx.future((Kind(provider), recheck), |(Kind(provider), _)| async move {
    background::run(move || Copies(Arc::new(probe::detect_agent(provider)))).await
  })
}

/// The check of `program` as `provider`'s CLI, again at each Check; none without a program.
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

/// The crumb back to Connections, *Add an agent provider*, and what a provider does while it runs on its own sign-in
/// (`Head` `mHwXV`; `10b` draws none on an API provider, whose warning line says it).
fn head(ctx: &mut Ctx, props: &AddAgentProps, on_provider: bool) -> Head {
  let navigation = Navigation::of(ctx);
  let project = props.project.open.project.id();
  Head {
    back: navigation.to(Route::Project(
      project,
      ProjectPage::Connections(ConnectionsView::default()),
    )),
    here: ctx.t("connections.dialog.addCrumb"),
    title: ctx.t("connections.dialog.addProviderTitle"),
    description: (!on_provider).then(|| ctx.t("connections.dialog.addProviderLead")),
  }
}
