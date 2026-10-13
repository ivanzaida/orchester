//! Add connection for an agent provider (`Connections · Add connection · Claude Code CLI · full page · dark`
//! `cEJy4`, the program and check states of `l2kjVF`): a page in the Connections tab, never a dialog. The
//! person chooses the CLI (the native kinds are not available yet), names the connection, and gets the
//! program found on the search path, which Orchester checks at once (its version, sign-in and models).
//! Only a copy found on the search path is offered: choosing another file is not built in this app.
//! Save connection waits for a check that recognized the CLI; one that is not signed in may be saved and
//! reads Sign-in required. The first reason it cannot be saved stays under the actions.

mod check;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureHandle, FutureState},
    i18n::I18n,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{AgentProvider, ConnectionAccount, ConnectionCheck},
  name::Name,
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::{ConnectionDraft, ProjectConnections, save_on_account_in},
    probe,
  },
  messages::{self, Message},
};

use self::check::{Checked, check_row, program_row};
use super::{
  data::{self, Saved},
  page::{Head, action_bar, page},
};
use crate::{
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

type SaveAction = FutureAction<(ConnectionDraft, ConnectionAccount), Saved, Message>;

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
    };
    let save = page.save.clone();
    let project = ctx.props::<AddAgentProps>().project.open.project.id();
    let navigation = Navigation::of(ctx);
    ctx.watch(&save.state(), move |state| {
      if let Some(Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        let view = ConnectionsView::new(ConnectionsTab::AgentProviders).with_selected(id);
        navigation.replace(Route::Project(project, ProjectPage::Connections(view)));
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
    let reason = self.reason(ctx, listed.data.as_ref(), &state, &save.state().get());
    let sections = vec![
      self.kind_section(ctx, listed.data.as_ref(), props.project.open.project.name().as_str()),
      self.connection_section(ctx, listed.data.as_ref(), (&found, &state), &accounts),
      after(ctx, provider, props.project.open.project.name().as_str()),
    ];
    let bar = self.bar(ctx, &props, (&save, program, &state, account), reason.as_deref());
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

  /// Fills the name with the suggested one until the person types their own.
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
    (save, program, state, account): (&SaveAction, Option<PathBuf>, &Checked, ConnectionAccount),
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
    let draft = match (program, state.check(), Name::new(&self.name.get())) {
      (Some(executable), Some(check), Ok(name)) if reason.is_none() => Some(ConnectionDraft {
        id: None,
        name,
        provider: self.kind.get().0,
        executable,
        check: check.clone(),
      }),
      _ => None,
    };
    let button = match (save.state().get().is_pending(), draft) {
      (true, _) => button.loading(),
      (false, Some(draft)) => {
        let save = save.clone();
        button.on_click(move || save.run((draft.clone(), account.clone())))
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
    let account = super::account::add_row(ctx, accounts, provider, &self.account);
    let check = check_row(ctx, provider, state, &self.recheck);
    section(
      "connection-section",
      &ctx.t("connections.dialog.connection"),
      None,
      vec![name, program, account, check],
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

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let props = ctx.props::<AddAgentProps>().clone();
  let activity = Services::of(ctx).activity;
  ctx.future_action(move |(draft, account): (ConnectionDraft, ConnectionAccount)| {
    let (activity, open) = (activity.clone(), props.project.open.clone());
    async move {
      let spec = operations::connection_save(&open, draft.name.as_str());
      let save = async move {
        background::run(move || save_on_account_in(&open.repo_root, draft, account).map(Saved))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot save the connection");
            error.message()
          })
      };
      activity.track(spec, save).await
    }
  })
}

/// `{CLI} · {project}`, numbered when another connection of the project has the name.
/// Puts `new` in the name while it holds the last suggestion or nothing: a name the person typed stays.
fn suggest(name: &Signal<String>, suggested: &Signal<String>, new: String) {
  let current = name.get_untracked();
  let previous = suggested.get_untracked();
  if (current.is_empty() || current == previous) && current != new {
    name.set(new.clone());
  }
  if previous != new {
    suggested.set(new);
  }
}

fn suggested_name(i18n: &I18n, listed: &ProjectConnections, provider: AgentProvider, project: &str) -> String {
  let short = i18n.t(short_key(provider));
  let taken = |name: &str| taken_names(listed).any(|taken| taken == name);
  let plain = i18n.t_args(
    "connections.dialog.suggestedName",
    [("provider", &*short), ("project", project)],
  );
  if !taken(&plain) {
    return plain.to_string();
  }
  (2_usize..)
    .map(|number| {
      i18n
        .t_args(
          "connections.dialog.suggestedNameNumbered",
          [
            ("provider", &*short),
            ("project", project),
            ("number", &number.to_string()),
          ],
        )
        .to_string()
    })
    .find(|name| !taken(name))
    .expect("only finitely many connection names are taken")
}

const fn short_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codexShort",
    AgentProvider::ClaudeCodeCli => "connections.provider.claudeShort",
    AgentProvider::AnthropicApi => "connections.provider.anthropicApiShort",
    AgentProvider::OpenAiApi => "connections.provider.openaiApiShort",
  }
}

/// The names the project's agent and GitHub connections have; they share one list.
fn taken_names(listed: &ProjectConnections) -> impl Iterator<Item = &str> {
  let agents = listed.connections.iter().map(|connection| connection.name().as_str());
  agents.chain(listed.github.iter().map(|connection| connection.name().as_str()))
}

/// Why `name` cannot be the connection's name.
fn name_problem(ctx: &Ctx, listed: &ProjectConnections, name: &str) -> Option<Arc<str>> {
  if let Err(error) = Name::new(name) {
    return Some(message_text(ctx, &messages::name_error(&error)));
  }
  taken_names(listed)
    .any(|taken| taken == name.trim())
    .then(|| ctx.t("connections.dialog.nameTaken"))
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
      Crumb::link(ctx.t("connections.title"), move || {
        navigation.go(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::default()),
        ));
      }),
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
