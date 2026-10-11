//! Secrets (ccz8n): metadata and references only. Values enter the masked inline form and the vault.
mod form;
mod inspector;
mod list;
mod page;
mod state;
mod words;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Row,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  secrets::data::ProjectSecrets,
  tasks::outside::Outside,
};
use state::{EnvironmentChoice, Panel, Selected, View};

use crate::{
  project::ShownProject,
  services::Services,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
struct Props(ShownProject);
impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<SecretsScreen>(
    &format!("secrets-{}", project.open.project.id()),
    Props(project.clone()),
  )
}

struct SecretsScreen {
  view: View,
}

impl Component for SecretsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: View {
        selected: ctx.signal(Selected(None)),
        panel: ctx.signal(Panel::Inspect),
        environment: ctx.signal(EnvironmentChoice(None)),
        revision: ctx.signal(0),
      },
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().0.clone();
    let revision = self.view.revision.get() + project.feed.outside.get().of(Outside::Connections);
    let selected = self.view.selected.get().0;
    let repo = project.open.repo_root.clone();
    let hold = Services::of(ctx).hold;
    let loaded = ctx
      .future((repo, revision, selected), move |(repo, _, selected)| {
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
    let page = page::frame().child(page::header(ctx, &project, &self.view));
    let count = loaded
      .data
      .as_ref()
      .map_or(0, |listed| listed.in_environment(None).count());
    let page = page.child(page::tabs(ctx, &self.view, count));
    if self.view.panel.get() == Panel::Usage {
      return page.child(page::usage(ctx));
    }
    if let Some(ref error) = loaded.error
      && !loaded.is_pending()
    {
      return page.child(self.failed(ctx, error));
    }
    let Some(listed) = loaded.data else {
      return page.child(match loaded.error {
        Some(ref error) if !loaded.is_pending() => self.failed(ctx, error),
        _ => loading("secrets-loading", &ctx.t("secrets.reading")).into(),
      });
    };
    page.child(self.body(ctx, &project, &listed))
  }
}

impl SecretsScreen {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.view.revision.clone();
    load_error(
      ctx,
      "secrets-load-error",
      (&ctx.t("secrets.loadFailed"), &message_text(ctx, error)),
      move || revision.update(|revision| *revision += 1),
    )
  }

  fn body(&self, ctx: &mut Ctx, project: &ShownProject, listed: &ProjectSecrets) -> Element {
    let environment = self.view.environment.get().0;
    let secret = self
      .view
      .selected
      .get()
      .0
      .and_then(|id| listed.secret(id))
      .filter(|secret| environment.is_none_or(|environment| secret.environment() == environment))
      .or_else(|| listed.in_environment(environment).next());
    let list = list::column(ctx, listed, &self.view, secret.map(|secret| secret.id()));
    let mut body = Row::new()
      .id("secrets-body")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .spacing(SpacingSize::Xl)
      .child(list);
    match (self.view.panel.get(), secret) {
      (Panel::Add, _) => {
        body = body.child(ctx.mount_keyed::<form::SecretForm>(
          "secret-new-form",
          form::Props {
            project: project.clone(),
            listed: listed.clone(),
            secret: None,
            view: self.view.clone(),
          },
        ));
      }
      (Panel::Replace, Some(secret)) => {
        body = body.child(ctx.mount_keyed::<form::SecretForm>(
          &format!("secret-replace-{}", secret.id()),
          form::Props {
            project: project.clone(),
            listed: listed.clone(),
            secret: Some(secret.clone()),
            view: self.view.clone(),
          },
        ));
      }
      (_, Some(secret)) => {
        body = body.child(ctx.mount_keyed::<inspector::Inspector>(
          &format!("secret-inspector-{}", secret.id()),
          inspector::Props {
            project: project.clone(),
            listed: listed.clone(),
            secret: secret.clone(),
            view: self.view.clone(),
            archive: self.view.panel.get() == Panel::Archive,
          },
        ))
      }
      _ => {}
    }
    body.into()
  }
}
