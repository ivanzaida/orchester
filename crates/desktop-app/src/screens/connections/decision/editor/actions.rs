//! Form saves and draft checks, with authority captured at input delivery.
use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  node::Element,
};
use orchester_core::{connection::DecisionConnection, secret::SecretValue};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    decision::{self, DecisionDraft, Writer},
    native::KeySecret,
  },
  messages::Message,
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::{DecisionEditor, Props};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::project_settings::kit::form_actions,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub(super) struct Start {
  open: OpenProject,
  draft: DecisionDraft,
  key: Option<KeySecret>,
  value: Option<SecretValue>,
  writer: Writer,
  save: bool,
  revision: u64,
  expected: Option<DecisionConnection>,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum Done {
  Saved(orchester_core::id::DecisionConnectionId),
  Checked(Box<Checked>),
}
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Checked {
  pub draft: DecisionDraft,
  pub key: Option<KeySecret>,
  pub connection: DecisionConnection,
  pub revision: u64,
}
impl DevtoolsInspectable for Done {}
pub(super) type Action = FutureAction<Start, Done, Message>;

pub(super) fn action(ctx: &mut Ctx) -> Action {
  let Services {
    vault,
    activity,
    changes,
    ..
  } = Services::of(ctx);
  ctx.future_action(move |start: Start| {
    let (vault, activity, changes) = (vault.clone(), activity.clone(), changes.clone());
    async move {
      let Start {
        open,
        draft,
        key,
        value,
        writer,
        save,
        revision,
        expected,
      } = start;
      let spec = if save {
        operations::connection_save(&open, draft.name.as_str())
      } else {
        operations::connection_check(&open, draft.name.as_str())
      };
      let root = open.repo_root.clone();
      activity
        .track(spec, async {
          if save {
            let id = background::run(move || {
              decision::save_with_key_in(
                &root,
                vault.get(),
                &draft,
                (key.as_ref(), value),
                writer,
                expected.as_ref(),
              )
            })
            .await??;
            changes.record(open.project.id(), Outside::Connections);
            changes.record(open.project.id(), Outside::ProjectSettings);
            Ok(Done::Saved(id))
          } else {
            let connection = decision::check_draft_in(&root, vault.get(), &draft, key.as_ref(), value, writer).await?;
            Ok(Done::Checked(Box::new(Checked {
              draft,
              key,
              connection,
              revision,
            })))
          }
        })
        .await
    }
  })
}

impl DecisionEditor {
  pub(super) fn check_button(&self, ctx: &Ctx, props: &Props) -> ActionButton {
    let button = ActionButton::new(
      "decision-edit-check",
      ButtonKind::Secondary,
      ctx.t("decision_connection.checkNow"),
    )
    .icon(Icon::RefreshCw);
    if self.work.state().get().is_pending() {
      return if self.checking.get() {
        button.loading()
      } else {
        button.disabled()
      };
    }
    let Some((draft, key)) = self.draft(props) else {
      return button.unavailable(ctx.t("decision_connection.completeFields"));
    };
    let (work, open, guard) = (
      self.work.clone(),
      props.project.open.clone(),
      self.fields.key.save_guard(),
    );
    let revision = self.fields.key.key_revision();
    // The value is read only when clicked, never captured into a visible/inspectable draft.
    let checking = self.checking.clone();
    button.on_click(move || {
      checking.set(true);
      work.run(Start {
        open: open.clone(),
        draft: draft.clone(),
        key: key.clone(),
        value: guard.typed_key(),
        writer: if lurq::app::events::is_synthetic_input() {
          Writer::Automation
        } else {
          Writer::Person
        },
        save: false,
        revision,
        expected: None,
      })
    })
  }

  pub(super) fn bar(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let hosted = props
      .connection
      .as_ref()
      .is_some_and(|connection| !connection.is_on_this_computer())
      || orchester_core::connection::DecisionServerUrl::new(&self.fields.url.get())
        .is_ok_and(|url| !url.is_on_this_computer());
    let close = props.close.clone();
    let cancel = ActionButton::new(
      "decision-edit-cancel",
      ButtonKind::Ghost,
      ctx.t("decision_connection.cancel"),
    )
    .on_click(move || close.set(false));
    let save = ActionButton::new(
      "decision-edit-save",
      ButtonKind::Primary,
      ctx.t("decision_connection.save"),
    )
    .icon(Icon::Check);
    let save = if self.work.state().get().is_pending() {
      if self.checking.get() {
        save.disabled()
      } else {
        save.loading()
      }
    } else if let Some((draft, key)) = self.draft(props) {
      let (work, open, guard, refused) = (
        self.work.clone(),
        props.project.open.clone(),
        self.fields.key.save_guard(),
        self.refused.clone(),
      );
      let gate = ctx.use_context::<InputGate>();
      let checking = self.checking.clone();
      let expected = self.original.get().cloned();
      save.on_click(move || {
        let synthetic = lurq::app::events::is_synthetic_input();
        // Hosted decision connections are strictly person-only, even with automation consent.
        if (hosted && synthetic)
          || (matches!(key, Some(KeySecret::New(_))) && !input_may_decide(gate.as_ref(), Guarded::PermissionWidening))
        {
          refused.set(true);
          return;
        }
        refused.set(false);
        checking.set(false);
        work.run(Start {
          open: open.clone(),
          draft: draft.clone(),
          key: key.clone(),
          value: guard.typed_key(),
          writer: if synthetic { Writer::Automation } else { Writer::Person },
          save: true,
          revision: 0,
          expected: expected.clone(),
        });
      })
    } else {
      save.unavailable(ctx.t("decision_connection.completeFields"))
    };
    let reason = self
      .error(ctx)
      .or_else(|| {
        self.refused.get().then(|| {
          ctx
            .t(if hosted {
              "decision_connection.personOnly"
            } else {
              "decision_connection.secretPersonOnly"
            })
            .to_string()
        })
      })
      .or_else(|| hosted.then(|| ctx.t("decision_connection.personOnly").to_string()));
    form_actions(
      "decision-edit-actions",
      vec![cancel.into(), save.into()],
      reason.as_deref(),
    )
  }
}
