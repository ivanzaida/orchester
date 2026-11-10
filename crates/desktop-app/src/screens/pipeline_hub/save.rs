//! What the bar of a pipeline's panel does: *Save* stores the edit the panel holds as the pipeline's draft and
//! publishes it as the next version at once ([`local_edit::save`]). A save that widens what the pipeline's agents may
//! do asks first, in the panel, and only a person's press answers it; a refusal says why in place, and a save that
//! could not publish says it kept the draft and why ([`Notes`]). A save ends with the screens that read the pipelines
//! reading them again.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::Pipeline,
};
use orchester_workspace::{
  agents::widening::answers_in_persons_place,
  background,
  messages::Message,
  pipelines::local_edit::{self, Held, Saved},
  tasks::outside::Outside,
};

use super::edit::Holding;
use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{NoticeProps, WidenLook, WidenNotice, WidenProps},
  services::Services,
  ui::{icon::Icon, message::message_text},
};

/// A save and what the person already allowed it to widen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ask {
  pub held: Held,
  pub allowed: Vec<Message>,
}

/// A save that ended, and what the workspace said of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Done {
  pub ask: Ask,
  pub saved: Saved,
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

/// A save that did not publish, which the bar says: the draft it kept and why.
pub(super) type Kept = Signal<Option<Message>>;

/// Stores and publishes what the panel holds of the pipeline `id` in the project at `root`.
pub(super) fn save(ctx: &mut Ctx, root: PathBuf, id: ResourceId) -> Save {
  ctx.future_action(move |ask: Ask| {
    let (root, asked) = (root.clone(), ask.clone());
    async move {
      let saved =
        background::run(move || local_edit::save(&root, id, asked.held.base(), asked.held.edit(), &asked.allowed))
          .await??;
      Ok(Done { ask, saved })
    }
  })
}

/// What a save that ended changes, which `follow` writes.
pub(super) struct Followed {
  pub pending: Pending,
  pub holding: Signal<Holding>,
  pub kept: Kept,
  /// The pipeline as stored now, which the panel keeps current.
  pub stored: lurq::core::Ref<Option<Arc<Pipeline>>>,
}

/// What follows a save that ended. One that stored the edit leaves the panel showing it as nothing changed, until the
/// hub lists what was stored, and is recorded for the screens that read the pipelines (a published one also for New
/// task's pipelines); a widening is asked.
pub(super) fn follow(ctx: &mut Ctx, save: &Save, followed: &Followed, project: ProjectId) {
  let changes = Services::of(ctx).changes;
  let Followed {
    pending,
    holding,
    kept,
    stored,
  } = followed;
  let (pending, holding, kept, stored) = (pending.clone(), holding.clone(), kept.clone(), stored.clone());
  ctx.watch(&save.state(), move |state| {
    if state.is_pending() {
      return;
    }
    let Some(done) = state.data.clone().filter(|_| state.is_fulfilled()) else {
      return;
    };
    match done.saved {
      Saved::Confirm(reasons) => {
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
      saved => {
        pending.set(None);
        kept.set(match &saved {
          Saved::Drafted(why) => Some(why.clone()),
          _ => None,
        });
        if let Some(body) = done.ask.held.edit().validate().ok().map(Arc::new) {
          holding.set(Holding {
            held: Some(Held::start(body)),
            saved_over: stored.get(),
          });
        }
        changes.record(project, Outside::Pipelines);
        if matches!(saved, Saved::Published(_)) {
          changes.record(project, Outside::Tasks);
        }
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
    id: "pipeline-hub-confirm".into(),
    look: WidenLook::Band,
    affirming: false,
    automation,
    title: ctx.t("pipeline_hub.bar.widenTitle"),
    reasons: reasons.into(),
    confirm: ctx.t("pipeline_hub.bar.allow"),
    keep: ctx.t("pipeline_hub.bar.keep"),
    on_keep: Arc::new(move || keep.set(None)),
    on_confirm: Arc::new(move || {
      confirm.1.set(None);
      confirm.0.run(confirm.2.clone());
    }),
  };
  ctx.mount_keyed::<WidenNotice>(
    "pipeline-hub-confirm",
    NoticeProps {
      question: props,
      confirm_icon: Icon::Check,
    },
  )
}
