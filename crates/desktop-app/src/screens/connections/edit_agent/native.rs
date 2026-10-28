//! Edit connection for a native connection (adapting the old design's native form 04.H1–H3 and models dialog 04.H6 to
//! this app's sub-page, as `10b` `hc1vS` draws a form): its name as it is, then its preset, base URL, key secret,
//! models, default model and effort, and provider fallback ([`NativeForm`]), with the line of where requests go. A
//! model the endpoint listed keeps its limits and efforts, and a model that a profile or a pipeline step runs on is
//! never dropped: saving one away is refused, saying who uses it (core's `ensure_used_kept`). Saving a connection with
//! a key is a person's decision, as adding one is.

use std::path::PathBuf;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  node::Element,
};
use orchester_core::{
  connection::{AgentConnection, ApiEndpoint},
  secret::SecretValue,
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::ProjectConnections,
    native::{KeySecret, NativeDraft, save_native_in},
  },
  messages::Message,
};

use super::{EditConnection, EditProps};
use crate::{
  mcp::input_gate::InputGate,
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route, Screen},
  screens::{
    connections::{
      data::Saved,
      page::{Head, action_bar_noted, page},
    },
    project_settings::kit::{Group, Tone, form_row, value},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    form_section::Label,
    icon::Icon,
    message::message_text,
  },
};

pub(super) type SaveNative = FutureAction<(PathBuf, NativeDraft, Option<SecretValue>), Saved, Message>;

impl EditConnection {
  /// The page of native connection `connection` on `api`.
  pub(super) fn native_page(
    &self,
    ctx: &mut Ctx,
    props: &EditProps,
    (connection, api, known): (&AgentConnection, &ApiEndpoint, &ProjectConnections),
  ) -> Element {
    self.native.seed(api);
    let provider = connection.provider();
    let mut rows = vec![form_row(
      Label::new("connection-edit-name", ctx.t("connections.dialog.name")),
      value("connection-edit-name-value", connection.name().as_str()),
    )];
    rows.extend(self.native.rows(ctx, provider, (known, Some(api.models()))));
    let sections = vec![
      Group::new("connection-edit-properties", ctx.t("connections.dialog.connection"))
        .rows(rows)
        .into(),
      self.native.warning(ctx),
    ];
    let draft = self.native.draft(
      known,
      Some(api.models()),
      (Some(connection.id()), connection.name().clone(), provider),
    );
    let changed = draft.as_ref().filter(|draft| differs(draft, api)).cloned();
    let head = Head {
      description: Some(ctx.t("connections.native.editDescription")),
      ..self.head(ctx, props, connection)
    };
    let bar = self.native_bar(ctx, props, connection, (changed, known));
    page(ctx, Screen::EditConnection, head, sections, bar)
  }

  /// *Cancel* back to the connection, borderless as in 05c and 10c–10f, and *Save connection*, which stores what the
  /// fields hold once something changed (05j `k8IU1`).
  fn native_bar(
    &self,
    ctx: &mut Ctx,
    props: &EditProps,
    connection: &AgentConnection,
    (changed, known): (Option<NativeDraft>, &ProjectConnections),
  ) -> Element {
    let navigation = Navigation::of(ctx);
    let (project, id) = (props.project.open.project.id(), connection.id());
    let cancel =
      ActionButton::new("connection-edit-cancel", ButtonKind::Ghost, ctx.t("connections.cancel")).on_click(move || {
        navigation.go(Route::Project(
          project,
          ProjectPage::Connections(ConnectionsView::of(id)),
        ));
      });
    let save = ActionButton::new(
      "connection-edit-save",
      ButtonKind::Primary,
      ctx.t("connections.editAgent.save"),
    )
    .icon(Icon::Check);
    let state = self.save_native.state().get();
    let save = match changed {
      _ if state.is_pending() => save.loading(),
      Some(draft) => {
        let (action, guard, repo) = (
          self.save_native.clone(),
          self.native.save_guard(),
          props.project.open.repo_root.clone(),
        );
        let gate = ctx.use_context::<InputGate>();
        save.on_click(move || {
          if let Some(value) = guard.admit_key(&draft.key, gate.as_ref()) {
            action.run((repo.clone(), draft.clone(), value));
          }
        })
      }
      None => save.disabled(),
    };
    let rejected = state.is_rejected();
    let failed = state.error.filter(|_| rejected).map(|error| {
      ctx.t_args(
        "connections.dialog.reasons.failed",
        [("reason", &*message_text(ctx, &error))],
      )
    });
    let reason = failed.or_else(|| self.native.reason(ctx, known));
    let note = match (self.native.refusal(ctx), reason) {
      (Some(refusal), _) => Some((refusal, Tone::Warning)),
      (None, reason) => reason.map(|reason| (reason, Tone::Muted)),
    };
    let note = note.as_ref().map(|(words, tone)| (&**words, *tone));
    action_bar_noted(vec![cancel.into(), save.into()], note)
  }
}

/// Whether `draft` stores anything other than `api` holds now.
fn differs(draft: &NativeDraft, api: &ApiEndpoint) -> bool {
  let key_changed = match &draft.key {
    KeySecret::Existing(key) => *key != api.key(),
    KeySecret::New(_) => true,
  };
  key_changed
    || draft.preset != api.preset()
    || draft.base_url != *api.base_url()
    || draft.models != *api.models()
    || draft.provider_fallback != api.provider_fallback()
}

/// Saves the native connection a draft describes in the project at its path, with the key typed for a new secret,
/// listed in the status bar's activity.
pub(super) fn save_action(ctx: &mut Ctx) -> SaveNative {
  let props = ctx.props::<EditProps>().clone();
  let services = Services::of(ctx);
  let (activity, vault) = (services.activity, services.vault);
  ctx.future_action(
    move |(repo, draft, value): (PathBuf, NativeDraft, Option<SecretValue>)| {
      let (activity, vault, open) = (activity.clone(), vault.clone(), props.project.open.clone());
      async move {
        let spec = operations::connection_save(&open, draft.name.as_str());
        let save =
          async move { background::run(move || save_native_in(&repo, vault.get(), &draft, value).map(Saved)).await? };
        activity.track(spec, save).await
      }
    },
  )
}
