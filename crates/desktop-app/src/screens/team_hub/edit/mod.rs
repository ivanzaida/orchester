//! An admin's edit of a team instruction, skill, tool or profile in its panel, in place (`Agents · v2` `acFkz` A26–A31,
//! `docs/design/team-configuration-live.md`, rule 6): the panel's fields hold the latest version read this session, and
//! the fixed bar's Save publishes the body they make as the next version on the team server at once, made on the
//! version the fields were opened on; Cancel puts that version back in them. Nothing is kept on this computer. A
//! version someone published first is refused, and the fields reload from the server's latest, which the notice over
//! them says (A30); while the team server cannot be reached what was typed stays and Save waits for it (A31).
//!
//! The kind's own fields are its panel's ([`super::plain`], [`super::tool`]); this is what they share: the version they
//! are made on, Save's action and what it answered ([`view`]).

mod fields;
mod view;

use std::{fmt::Debug, future::Future, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::{Instruction, Skill, TeamAgentProfile, Tool, VersionNumber},
};
use orchester_workspace::{
  agents::team_body_edit::{self, Latest},
  messages::Message,
  sharing::{Servers, team_edit::EditRefused},
  tasks::outside::Outside,
  team_config::TeamConfig,
};
use parking_lot::Mutex;

pub(super) use self::{
  fields::TextSection,
  view::{Parts, View},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  services::Services,
};

/// A team resource's body an admin edits in its panel.
pub(super) trait Body: Clone + PartialEq + Debug + Send + Sync + 'static {
  /// The screens that show what Save changes.
  const OUTSIDE: Outside;

  /// What an edit of team resource `id` starts from as `config` holds it now.
  fn latest(config: &TeamConfig, id: ResourceId) -> Option<Latest<Self>>;

  /// Publishes `body` as the next version of team resource `id` of the project at `root`, made on version `base`.
  fn publish(
    servers: Servers,
    root: PathBuf,
    target: (ResourceId, VersionNumber),
    body: Self,
  ) -> impl Future<Output = Result<VersionNumber, EditRefused>> + Send;
}

macro_rules! team_body {
  ($body:ty, $outside:expr) => {
    impl Body for $body {
      const OUTSIDE: Outside = $outside;

      fn latest(config: &TeamConfig, id: ResourceId) -> Option<Latest<Self>> {
        team_body_edit::latest(config, id)
      }

      async fn publish(
        servers: Servers,
        root: PathBuf,
        target: (ResourceId, VersionNumber),
        body: Self,
      ) -> Result<VersionNumber, EditRefused> {
        team_body_edit::save(&servers, &root, target, &body).await
      }
    }
  };
}

team_body!(Instruction, Outside::Instructions);
team_body!(Skill, Outside::Skills);
team_body!(Tool, Outside::Tools);
team_body!(TeamAgentProfile, Outside::Profiles);

/// What the fields take when they are put back or reloaded: every field takes the body's value.
pub(super) type Load<B> = Arc<dyn Fn(&B) + Send + Sync>;

/// A press of Save: the body the fields make, the version it is made on, and whether a person (or automation they let
/// change what agents may do) pressed.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Asked<B> {
  body: B,
  base: VersionNumber,
  decided: bool,
}
impl<B> DevtoolsInspectable for Asked<B> {}

/// What Save published: the version the server numbered, with the body sent.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Saved<B>(Latest<B>);
impl<B> DevtoolsInspectable for Saved<B> {}

/// Why Save did not publish, with what the fields start from again when someone published first.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NotSaved<B> {
  refused: EditRefused,
  reload: Option<Latest<B>>,
}
impl<B> DevtoolsInspectable for NotSaved<B> {}

type Save<B> = FutureAction<Asked<B>, Saved<B>, NotSaved<B>>;

/// The resource an edit saves: the project's folder and id, and the resource's id.
pub(super) type Target = (PathBuf, ProjectId, ResourceId);

/// What a panel keeps of an admin's edit. Cloning shares it.
#[derive(Clone)]
pub(super) struct Edit<B: Body> {
  /// The version the fields were opened on: Save is made on it, and the fields are compared with its body.
  base: Arc<Mutex<Option<Latest<B>>>>,
  /// Counts the changes of `base`, so the panel draws again.
  based: Signal<u64>,
  /// Why the last Save did not publish: until the next press or Cancel.
  refused: Signal<Option<EditRefused>>,
  save: Save<B>,
  load: Load<B>,
  gate: Option<InputGate>,
}

impl<B: Body> Edit<B> {
  /// An edit of `target` whose fields start from `latest`, which `load` puts in them.
  pub(super) fn new(ctx: &mut Ctx, target: Target, latest: Option<&Latest<B>>, load: Load<B>) -> Self {
    if let Some(latest) = latest {
      load(&latest.body);
    }
    let edit = Self {
      base: Arc::new(Mutex::new(latest.cloned())),
      based: ctx.signal(0),
      refused: ctx.signal(None),
      save: save(ctx, target),
      load,
      gate: ctx.use_context::<InputGate>(),
    };
    edit.follow(ctx);
    edit
  }

  /// The version the fields are made on.
  pub(super) fn base(&self) -> Option<Latest<B>> {
    self.based.get();
    self.base.lock().clone()
  }

  /// Takes the answer of a Save: published, the fields are made on what was published; refused, the refusal is kept,
  /// and a version someone published first is what the fields show and are made on from now on.
  fn follow(&self, ctx: &mut Ctx) {
    let edit = self.clone();
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        edit.refused.set(None);
      } else if let Some(Saved(saved)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        edit.rebase(saved, false);
      } else if let Some(NotSaved { refused, reload }) = state.error.clone() {
        if let Some(reload) = reload {
          edit.rebase(reload, true);
        }
        edit.refused.set(Some(refused));
      }
    });
  }

  /// The fields are made on `latest` from now on, and take its body when `load`.
  fn rebase(&self, latest: Latest<B>, load: bool) {
    if load {
      (self.load)(&latest.body);
    }
    *self.base.lock() = Some(latest);
    self.based.update(|n| *n += 1);
  }

  /// Keeps the fields on `latest`, the version the panel reads now, while nothing in them differs from the version
  /// they were made on: a version published elsewhere shows at once. Changed fields stay; Save then says someone
  /// published first.
  pub(super) fn keep_up(&self, latest: Option<&Latest<B>>, changed: bool) {
    let Some(latest) = latest else {
      return;
    };
    let behind = self.base.lock().as_ref().is_none_or(|base| base.number < latest.number);
    if behind && !changed && !self.save.state().get_untracked().is_pending() {
      self.rebase(latest.clone(), true);
    }
  }

  /// Cancel: the fields take the version they are made on again.
  fn cancel(&self) {
    if let Some(base) = self.base.lock().clone() {
      (self.load)(&base.body);
    }
    self.refused.set(None);
  }

  /// Save: a person's press (or automation's where they let it change what agents may do) sends `body`, made on
  /// `base`.
  fn press_save(&self, body: B, base: VersionNumber) {
    let decided = input_may_decide(self.gate.as_ref(), Guarded::PermissionWidening);
    self.save.run(Asked { body, base, decided });
  }
}

/// Save's action: the body goes to the team server as the next version of `id`, made on the version given; a version
/// someone published first comes back to reload the fields from.
fn save<B: Body>(ctx: &mut Ctx, (root, project, id): Target) -> Save<B> {
  let services = Services::of(ctx);
  ctx.future_action(move |Asked { body, base, decided }: Asked<B>| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let refused = |refused| NotSaved { refused, reload: None };
      if !decided {
        return Err(refused(EditRefused::person_only()));
      }
      if services.workspace.team_read(&root).config().is_none() {
        return Err(refused(EditRefused::Failed(Message::new("teamConfig.reading"))));
      }
      let sent = B::publish(services.servers.clone(), root.clone(), (id, base), body.clone()).await;
      services.changes.record(project, B::OUTSIDE);
      match sent {
        Ok(number) => Ok(Saved(Latest { number, body })),
        Err(changed @ EditRefused::Changed { .. }) => {
          let reload = services
            .workspace
            .team_read(&root)
            .config()
            .and_then(|config| B::latest(config, id));
          Err(NotSaved {
            refused: changed,
            reload,
          })
        }
        Err(other) => Err(refused(other)),
      }
    }
  })
}
