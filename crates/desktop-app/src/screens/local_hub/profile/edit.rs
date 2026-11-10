//! What the bar of a local profile's panel does: *Save* publishes what the panel holds (its choices, made by
//! [`super::choices`]) as the next version at once ([`local_edit::save`]). A save that widens what its agents may do
//! asks first, in the panel, and only a person's press answers it; a refusal says why in place. A save ends with the
//! screens that read the profiles reading them again.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::{Ref, Signal},
  node::Element,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::AgentProfile,
};
use orchester_workspace::{
  agents::{
    editor::Outcome,
    local_edit::{self, Held},
    local_hub::LocalProfile,
    widening::answers_in_persons_place,
  },
  background,
  messages::Message,
  tasks::outside::Outside,
};

use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{NoticeProps, WidenLook, WidenNotice, WidenProps},
  services::Services,
  ui::{icon::Icon, message::message_text},
};

/// What the panel holds of the profile: `None` while it holds its published version.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Holding(pub Option<Held>);
impl DevtoolsInspectable for Holding {}

impl Holding {
  /// The body the next choice is made on, and Save publishes: `None` for the published version.
  pub(super) fn body(&self) -> Option<AgentProfile> {
    self.0.as_ref().map(|held| held.body.clone())
  }
}

/// A save of what the panel holds and what the person already allowed it to widen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ask {
  pub held: AgentProfile,
  pub allowed: Vec<Message>,
}

/// A save that ended, and what the workspace said of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Done {
  pub ask: Ask,
  pub outcome: Outcome,
}
impl DevtoolsInspectable for Done {}

/// A save that waits for the person: what it widens beyond what they allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Asked {
  /// The save with everything it widens allowed, which the confirm runs.
  pub ask: Ask,
  pub reasons: Vec<Message>,
}

impl DevtoolsInspectable for Asked {}

pub(super) type Pending = Signal<Option<Asked>>;
pub(super) type Save = FutureAction<Ask, Done, Message>;
/// Publishes what the panel holds of the profile `id` in the project at `root`.
pub(super) fn save(ctx: &mut Ctx, root: PathBuf, id: ResourceId) -> Save {
  ctx.future_action(move |ask: Ask| {
    let (root, asked) = (root.clone(), ask.clone());
    async move {
      let outcome = background::run(move || local_edit::save(&root, id, &asked.held, &asked.allowed)).await??;
      Ok(Done { ask, outcome })
    }
  })
}

/// What a save, and the panel's state, that `follow` looks after.
pub(super) struct Followed {
  pub pending: Pending,
  pub held: Signal<Holding>,
  /// The profile as the hub lists it now, which the panel keeps current.
  pub now: Ref<Option<LocalProfile>>,
  /// The profile as the hub listed it when a save ended: the panel shows what it held until the hub lists another.
  pub when: Ref<Option<LocalProfile>>,
}

/// What follows a save that ended: a published one is recorded for the screens that read the profiles, and the panel
/// holds it as nothing changed; a widening is asked.
pub(super) fn follow(ctx: &mut Ctx, save: &Save, followed: &Followed, project: ProjectId) {
  let changes = Services::of(ctx).changes;
  let Followed {
    pending,
    held,
    now,
    when,
  } = followed;
  let (pending, held, now, when) = (pending.clone(), held.clone(), now.clone(), when.clone());
  ctx.watch(&save.state(), move |state| {
    if state.is_pending() {
      return;
    }
    let Some(done) = state.data.clone().filter(|_| state.is_fulfilled()) else {
      return;
    };
    match done.outcome {
      Outcome::Saved(_) => {
        pending.set(None);
        when.set(now.get());
        held.update(|holding| {
          if let Some(held) = holding.0.as_mut() {
            held.changed = false;
          }
        });
        for change in [Outside::Profiles, Outside::Pipelines] {
          changes.record(project, change);
        }
      }
      Outcome::Confirm(reasons) => {
        let asked = reasons
          .iter()
          .filter(|reason| !done.ask.allowed.contains(reason))
          .cloned()
          .collect();
        pending.set(Some(Asked {
          ask: Ask {
            allowed: reasons,
            ..done.ask
          },
          reasons: asked,
        }));
      }
    }
  });
}

/// The question of a save that widens, in the panel: what it widens, *Allow* and *Keep editing*.
pub(super) fn question(ctx: &mut Ctx, asked: &Asked, (save, pending): (&Save, &Pending)) -> Element {
  let automation = if answers_in_persons_place(&asked.reasons) {
    Guarded::ApprovalWidening
  } else {
    Guarded::PermissionWidening
  };
  let reasons: Vec<Arc<str>> = asked.reasons.iter().map(|reason| message_text(ctx, reason)).collect();
  let (keep, confirm) = (pending.clone(), (save.clone(), pending.clone(), asked.ask.clone()));
  let props = WidenProps {
    id: "local-hub-profile-confirm".into(),
    look: WidenLook::Band,
    affirming: false,
    automation,
    title: ctx.t("local_hub.edit.widenTitle"),
    reasons: reasons.into(),
    confirm: ctx.t("local_hub.edit.allow"),
    keep: ctx.t("local_hub.edit.keep"),
    on_keep: Arc::new(move || keep.set(None)),
    on_confirm: Arc::new(move || {
      confirm.1.set(None);
      confirm.0.run(confirm.2.clone());
    }),
  };
  ctx.mount_keyed::<WidenNotice>(
    "local-hub-profile-confirm",
    NoticeProps {
      question: props,
      confirm_icon: Icon::Check,
    },
  )
}
