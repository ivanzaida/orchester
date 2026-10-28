//! Add provider for the Anthropic API's native kind (`docs/architecture/native-agent.md` §7, the old design's native
//! form 04.H1–H3 in this page's rows): the Kind group, then the Connection group with the name and the native fields
//! ([`NativeForm`]), the line of where requests go, and Save connection. No program is looked for or checked: the
//! connection is tested on its page once saved, through the project's runner, which is also where its models are
//! fetched. A connection with a key is a person's decision, so automation's input does not save it.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  node::Element,
};
use orchester_core::{name::Name, secret::SecretValue};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::ProjectConnections,
    native::{NativeDraft, save_native_in},
  },
  messages::Message,
};

use super::{AddAgent, AddAgentProps, head, naming::name_problem};
use crate::{
  mcp::input_gate::InputGate,
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route, Screen},
  screens::{
    connections::{
      data::Saved,
      page::{action_bar_noted, page},
    },
    project_settings::kit::{Tone, form_group},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

pub(super) type SaveNative = FutureAction<(NativeDraft, Option<SecretValue>), Saved, Message>;

impl AddAgent {
  /// The page while a native kind is chosen.
  pub(super) fn native_page(
    &self,
    ctx: &mut Ctx,
    props: &AddAgentProps,
    listed: &FutureState<ProjectConnections, Message>,
  ) -> Element {
    let provider = self.kind.get().0;
    if let Some(listed) = listed.fulfilled_data() {
      let project = props.project.open.project.name().as_str();
      let new = super::naming::suggested_name(ctx.i18n(), listed, provider, project);
      super::naming::suggest(&self.name, &self.suggested, new);
    }
    let known = listed.data.clone().unwrap_or_default();
    let mut rows = vec![self.name_row(ctx, listed.data.as_ref())];
    rows.extend(self.native.rows(ctx, provider, (&known, None)));
    let sections = vec![
      self.kind_section(ctx, listed.data.as_ref(), props.project.open.project.name().as_str()),
      form_group(
        "connection-section",
        &ctx.t("connections.dialog.connection"),
        None,
        rows,
      ),
      self.native.warning(ctx),
    ];
    let reason = self.native_reason(ctx, listed.data.as_ref());
    let bar = self.native_bar(ctx, props, &known, reason.as_deref());
    let head = head(ctx, props, false);
    page(ctx, Screen::NewConnection, head, sections, bar)
  }

  /// The first reason Save connection cannot be used, in field order; `None` once it can.
  fn native_reason(&self, ctx: &Ctx, listed: Option<&ProjectConnections>) -> Option<Arc<str>> {
    let saving = self.save_native.state().get();
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
    self.native.reason(ctx, listed)
  }

  /// Cancel and Save connection, after why Save cannot be used or, once it can, what comes after saving.
  fn native_bar(
    &self,
    ctx: &mut Ctx,
    props: &AddAgentProps,
    known: &ProjectConnections,
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
    let draft = Name::new(&self.name.get())
      .ok()
      .filter(|_| reason.is_none())
      .and_then(|name| self.native.draft(known, None, (None, name, self.kind.get().0)));
    let button = match (self.save_native.state().get().is_pending(), draft) {
      (true, _) => button.loading(),
      (false, Some(draft)) => {
        let (save, guard) = (self.save_native.clone(), self.native.save_guard());
        let gate = ctx.use_context::<InputGate>();
        button.on_click(move || {
          if let Some(value) = guard.admit_key(&draft.key, gate.as_ref()) {
            save.run((draft.clone(), value));
          }
        })
      }
      (false, None) => button.disabled(),
    };
    let refusal = self.native.refusal(ctx);
    let next = ctx.t("connections.native.nextStep");
    let note = match refusal.as_deref().or(reason) {
      Some(reason) => (reason.to_owned(), Tone::Muted),
      None => (next.to_string(), Tone::Success),
    };
    action_bar_noted(vec![cancel.into(), button.into()], Some((note.0.as_str(), note.1)))
  }
}

/// Saves the native connection a draft describes, with the key typed for a new secret, listed in the status bar's
/// activity.
pub(super) fn save_action(ctx: &mut Ctx) -> SaveNative {
  let props = ctx.props::<AddAgentProps>().clone();
  let services = Services::of(ctx);
  let (activity, vault) = (services.activity, services.vault);
  ctx.future_action(move |(draft, value): (NativeDraft, Option<SecretValue>)| {
    let (activity, open, vault) = (activity.clone(), props.project.open.clone(), vault.clone());
    async move {
      let spec = operations::connection_save(&open, draft.name.as_str());
      let save = async move {
        background::run(move || save_native_in(&open.repo_root, vault.get(), &draft, value).map(Saved)).await?
      };
      activity.track(spec, save).await
    }
  })
}
