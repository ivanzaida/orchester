//! Real model-list checks and deletion, with the same workspace boundary as automation.
use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{id::DecisionConnectionId, resource::ModelId};
use orchester_workspace::{
  activity::operations, background, connections::decision, messages::Message, tasks::outside::Outside,
  workspace::OpenProject,
};

use crate::services::Services;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Work {
  Check(Option<ModelId>),
  Delete,
}

pub(super) type Action = FutureAction<(OpenProject, DecisionConnectionId, String, Work), bool, Message>;

pub(super) fn action(ctx: &mut Ctx) -> Action {
  let Services {
    vault,
    activity,
    changes,
    ..
  } = Services::of(ctx);
  ctx.future_action(
    move |(open, id, name, work): (OpenProject, DecisionConnectionId, String, Work)| {
      let (vault, activity, changes) = (vault.clone(), activity.clone(), changes.clone());
      async move {
        let spec = match work {
          Work::Check(_) => operations::connection_check(&open, &name),
          Work::Delete => operations::connection_save(&open, &name),
        };
        let removed = matches!(work, Work::Delete);
        let root = open.repo_root.clone();
        let run = async {
          match work {
            Work::Check(selected) => decision::check_in(&open.repo_root, vault.get(), id, selected.as_ref())
              .await
              .map(|_| ())
              .map_err(|error| error.message()),
            Work::Delete => {
              background::run(move || decision::delete_in(&root, id).map_err(|error| error.message())).await?
            }
          }
        };
        activity.track(spec, run).await?;
        changes.record(open.project.id(), Outside::Connections);
        Ok(removed)
      }
    },
  )
}
