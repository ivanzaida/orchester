//! What the bar of a pipeline's panel does: *Save* stores the edit the panel holds as the pipeline's draft and
//! publishes it as the next version at once ([`local_edit::save`]). A save that widens what the pipeline's agents may
//! do asks first, in the panel, and only a person's press answers it; a refusal says why in place, and a save that
//! could not publish says it kept the draft and why, and one that did says which version it made ([`Notes`]). A save
//! ends with the screens that read the pipelines reading them again.

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
  resource::{Pipeline, PipelineEdit, VersionNumber},
};
use orchester_workspace::{
  agents::widening::answers_in_persons_place,
  background,
  messages::Message,
  pipelines::local_edit::{self, Held, Saved},
  tasks::outside::Outside,
};

use super::{edit::Holding, leave::Published};
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

/// What the last save said, over the bar, until the edit changes or is put back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Told {
  /// A save that did not publish: the draft it kept, and why.
  pub kept: Option<Message>,
  /// A save that made this version.
  pub made: Option<VersionNumber>,
  /// A refused save: why, and the edit it refused.
  pub refused: Option<(Message, PipelineEdit)>,
  /// The edit the save under way was asked to store, which a refusal belongs to.
  tried: Option<PipelineEdit>,
}
impl DevtoolsInspectable for Told {}

impl Told {
  fn says_nothing(&self) -> bool {
    self.kept.is_none() && self.made.is_none() && self.refused.is_none()
  }
}

/// [`Told`], and how many saves stored the edit (the description's field is mounted anew after each, which takes the
/// focus off it).
#[derive(Clone)]
pub(super) struct Notes {
  told: Signal<Told>,
  pub stored: Signal<u64>,
}

impl Notes {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      told: ctx.signal(Told::default()),
      stored: ctx.signal(0),
    }
  }

  /// What the last save said. Reading it in a render subscribes the panel to it.
  pub(super) fn told(&self) -> Told {
    self.told.get()
  }

  /// A save of `edit` begins: what the last one said is over.
  pub(super) fn begin(&self, edit: &PipelineEdit) {
    self.told.set(Told {
      tried: Some(edit.clone()),
      ..Told::default()
    });
  }

  /// Forgets everything the last save said.
  pub(super) fn forget(&self) {
    if !self.told.get_untracked().says_nothing() {
      self.told.update(|told| *told = Told::default());
    }
  }

  /// The panel shows the changed edit `shown`: a save that kept a draft or made a version said it of an edit that is
  /// gone, and a refusal belongs to the edit it refused.
  pub(super) fn forget_stale(&self, shown: &PipelineEdit) {
    let told = self.told.get_untracked();
    let refusal_holds = told.refused.as_ref().is_some_and(|(_, refused)| refused == shown);
    if told.kept.is_some() || told.made.is_some() || (told.refused.is_some() && !refusal_holds) {
      self.told.update(|told| {
        told.kept = None;
        told.made = None;
        if !refusal_holds {
          told.refused = None;
        }
      });
    }
  }

  /// The save under way was refused for `why`.
  fn refuse(&self, why: Message) {
    self
      .told
      .update(|told| told.refused = told.tried.take().map(|tried| (why, tried)));
  }

  /// The save stored the edit, and `saved` is what it did with it.
  fn stored_as(&self, saved: &Saved) {
    self.stored.update(|count| *count += 1);
    self.told.update(|told| {
      told.kept = match saved {
        Saved::Drafted(why) => Some(why.clone()),
        _ => None,
      };
      told.made = match saved {
        Saved::Published(number) => Some(*number),
        _ => None,
      };
    });
  }
}

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
  pub notes: Notes,
  /// The pipeline as stored now, which the panel keeps current.
  pub stored: Ref<Option<Arc<Pipeline>>>,
  /// Where the screen hears that the pipeline was published, and which pipeline it is.
  pub published: (Signal<Published>, ResourceId),
}

/// What follows a save that ended. One that stored the edit leaves the panel showing it as nothing changed, until the
/// hub lists what was stored, and is recorded for the screens that read the pipelines (a published one also for New
/// task's pipelines); a widening is asked.
pub(super) fn follow(ctx: &mut Ctx, save: &Save, followed: &Followed, project: ProjectId) {
  let changes = Services::of(ctx).changes;
  let Followed {
    pending,
    holding,
    notes,
    stored,
    published,
  } = followed;
  let (pending, holding, notes, stored) = (pending.clone(), holding.clone(), notes.clone(), stored.clone());
  let published = published.clone();
  ctx.watch(&save.state(), move |state| {
    if state.is_pending() {
      return;
    }
    if state.is_rejected() {
      if let Some(why) = state.error.clone() {
        notes.refuse(why);
      }
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
        notes.stored_as(&saved);
        let body = done
          .ask
          .held
          .edit()
          .validate()
          .expect("a save that stored the edit validated it first");
        holding.set(Holding {
          held: Some(Held::start(Arc::new(body))),
          saved_over: stored.get(),
        });
        changes.record(project, Outside::Pipelines);
        if matches!(saved, Saved::Published(_)) {
          changes.record(project, Outside::Tasks);
          published.0.set(Published(Some(published.1)));
        }
      }
    }
  });
}

/// The question of a save that widens, in the panel: what it widens, *Allow* and *Keep editing*. After *Restore*
/// (`restored`) its title names the version the changes come from.
pub(super) fn question(
  ctx: &mut Ctx,
  asked: &Asked,
  (save, pending): (&Save, &Pending),
  restored: Option<VersionNumber>,
) -> Element {
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
    title: restored.map_or_else(
      || ctx.t("pipeline_hub.bar.widenTitle"),
      |version| {
        ctx.t_args(
          "pipeline_hub.bar.widenTitleRestored",
          [("version", version.to_string())],
        )
      },
    ),
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
