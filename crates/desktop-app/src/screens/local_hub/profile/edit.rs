//! What the select rows and the draft bar of a local profile's panel do: each change of a field saves the profile's
//! draft and *Publish vN* publishes it ([`orchester_workspace::agents::local_edit`], the editor's own rules). A change
//! that widens what its agents may do asks first, in the panel, and only a person's press answers it; a refusal says
//! why in place. Each write ends with the screens that read the profiles reading them again.

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
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    editor::Outcome,
    local_edit::{self, Edit},
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

/// What a press asks the workspace to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Do {
  /// Change one field of the draft.
  Edit(Edit),
  /// Publish the draft the panel showed, saved at `shown`: another draft is refused.
  Publish { shown: Timestamp },
}

/// A write and what the person already allowed it to widen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ask {
  pub what: Do,
  pub allowed: Vec<Message>,
}

/// A write that ended, and what the workspace said of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Done {
  pub ask: Ask,
  pub outcome: Outcome,
}
impl DevtoolsInspectable for Done {}

/// A write that waits for the person: what it widens beyond what they allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Asked {
  /// The write with everything it widens allowed, which the confirm runs.
  pub ask: Ask,
  pub reasons: Vec<Message>,
}

impl DevtoolsInspectable for Asked {}

pub(super) type Pending = Signal<Option<Asked>>;
pub(super) type Save = FutureAction<Ask, Done, Message>;

/// Saves a field of the draft of the profile `id` in the project at `root`, or publishes the draft.
pub(super) fn save(ctx: &mut Ctx, root: PathBuf, id: ResourceId) -> Save {
  ctx.future_action(move |ask: Ask| {
    let (root, asked) = (root.clone(), ask.clone());
    async move {
      let outcome = background::run(move || match &asked.what {
        Do::Edit(edit) => local_edit::edit(&root, id, edit, &asked.allowed),
        Do::Publish { shown } => local_edit::publish(&root, id, *shown, &asked.allowed),
      })
      .await??;
      Ok(Done { ask, outcome })
    }
  })
}

/// What follows a write that ended: a saved one is recorded for the screens that read the profiles, a widening is
/// asked, and `settled` counts it so that every select shows what is stored again.
pub(super) fn follow(ctx: &mut Ctx, save: &Save, pending: &Pending, settled: &Signal<u64>, project: ProjectId) {
  let changes = Services::of(ctx).changes;
  let (pending, settled) = (pending.clone(), settled.clone());
  ctx.watch(&save.state(), move |state| {
    if state.is_pending() {
      return;
    }
    settled.update(|count| *count += 1);
    let Some(done) = state.data.clone().filter(|_| state.is_fulfilled()) else {
      return;
    };
    match done.outcome {
      Outcome::Saved(_) => {
        pending.set(None);
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

/// The question of a write that widens, in the panel: what it widens, *Allow* and *Keep editing*.
pub(super) fn question(
  ctx: &mut Ctx,
  asked: &Asked,
  (save, pending): (&Save, &Pending),
  settled: &Signal<u64>,
) -> Element {
  let automation = if answers_in_persons_place(&asked.reasons) {
    Guarded::ApprovalWidening
  } else {
    Guarded::PermissionWidening
  };
  let reasons: Vec<Arc<str>> = asked.reasons.iter().map(|reason| message_text(ctx, reason)).collect();
  let (keep, confirm) = (
    (pending.clone(), settled.clone()),
    (save.clone(), pending.clone(), asked.ask.clone()),
  );
  let props = WidenProps {
    id: "local-hub-profile-confirm".into(),
    look: WidenLook::Band,
    affirming: false,
    automation,
    title: ctx.t("local_hub.edit.widenTitle"),
    reasons: reasons.into(),
    confirm: ctx.t("local_hub.edit.allow"),
    keep: ctx.t("local_hub.edit.keep"),
    on_keep: Arc::new(move || {
      keep.0.set(None);
      keep.1.update(|count| *count += 1);
    }),
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
