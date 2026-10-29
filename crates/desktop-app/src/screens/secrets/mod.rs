//! Project settings › Secrets (`06 Secrets` `bVh1e`, nothing yet `08` `aalCu`): metadata and references only. The
//! secrets by environment with what uses each and when its value was set (`list`), a secret's page with its details,
//! what uses it, its recent access, and Replace value and Archive asked in place (`06b` `famwr`, `inspector`), and
//! New secret as a page under the tab (`form`). Values enter the masked form and the vault, and are never shown again.
mod form;
mod inspector;
mod list;
mod state;
mod words;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  secrets::data::ProjectSecrets,
  tasks::outside::Outside,
};
use state::{EnvironmentChoice, View};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{SecretsPanel, SecretsView},
  screens::project_settings::kit,
  services::Services,
  ui::{
    message::message_text,
    state::load_error,
  },
};

#[derive(Clone, PartialEq)]
struct Props(ShownProject, SecretsView);
impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, shown: SecretsView) -> Element {
  ctx.mount_keyed::<SecretsScreen>(
    &format!("secrets-{}", project.open.project.id()),
    Props(project.clone(), shown),
  )
}

struct SecretsScreen {
  view: View,
}

impl Component for SecretsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let Props(project, shown) = ctx.props::<Props>().clone();
    let view = View {
      project: project.open.project.id(),
      navigation: Navigation::of(ctx),
      environment: ctx.signal(EnvironmentChoice(shown.environment)),
      revision: ctx.signal(0),
    };
    let chosen = view.clone();
    ctx.watch(&view.environment, move |choice| chosen.filter(choice.0));
    Self { view }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props(project, shown) = ctx.props::<Props>().clone();
    // Back and Forward move the filter too: the segments follow the route.
    if self.view.environment.get_untracked().0 != shown.environment {
      self.view.environment.set(EnvironmentChoice(shown.environment));
    }
    let revision = self.view.revision.get() + project.feed.outside.get().of(Outside::Connections);
    let repo = project.open.repo_root.clone();
    let hold = Services::of(ctx).hold;
    let loaded = ctx
      .future((repo, revision, shown.secret), move |(repo, _, selected)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || ProjectSecrets::load_selected_in(&repo, selected))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      })
      .state()
      .get();
    let Some(listed) = loaded
      .data
      .clone()
      .filter(|_| loaded.error.is_none() || loaded.is_pending())
    else {
      return kit::scroll(match loaded.error {
        Some(ref error) if !loaded.is_pending() => self.failed(ctx, error),
        _ => list::reading(ctx, &self.view),
      });
    };
    kit::scroll(self.body(ctx, (&project, shown), &listed))
  }
}

impl SecretsScreen {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.view.revision.clone();
    load_error(
      ctx,
      "secrets-load-error",
      (
        &ctx.t("secrets.loadFailed"),
        &ctx.t_args("secrets.loadFailureDetail", [("reason", message_text(ctx, error))]),
      ),
      move || revision.update(|revision| *revision += 1),
    )
  }

  /// New secret, the page of the secret the route names, or the list.
  fn body(&self, ctx: &mut Ctx, (project, shown): (&ShownProject, SecretsView), listed: &ProjectSecrets) -> Element {
    if shown.panel == SecretsPanel::Add {
      return ctx.mount_keyed::<form::SecretForm>(
        "secret-new-form",
        form::Props {
          project: project.clone(),
          listed: listed.clone(),
          secret: None,
          view: self.view.clone(),
        },
      );
    }
    match shown.secret.and_then(|id| listed.secret(id)) {
      Some(secret) => ctx.mount_keyed::<inspector::Inspector>(
        &format!("secret-inspector-{}", secret.id()),
        inspector::Props {
          project: project.clone(),
          listed: listed.clone(),
          secret: secret.clone(),
          view: self.view.clone(),
          panel: shown.panel,
        },
      ),
      None => list::list(ctx, listed, &self.view),
    }
  }
}
