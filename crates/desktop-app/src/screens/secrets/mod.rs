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
  components::{Column, Row},
  node::{Element, dimension::Dimension},
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
  routes::{SecretsPanel, SecretsTab, SecretsView},
  services::Services,
  ui::{
    message::message_text,
    state::{load_error, loading},
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
    // Back and Forward move the filter too: the select follows the route.
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
    let page = page::frame().child(page::header(ctx, &project, &self.view));
    let count = if loaded.error.is_some() && !loaded.is_pending() {
      None
    } else {
      loaded.data.as_ref().map(|listed| listed.in_environment(None).count())
    };
    let page = page.child(page::tabs(ctx, &self.view, shown.tab, count));
    if shown.tab == SecretsTab::Usage {
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
    page.child(self.body(ctx, (&project, shown), &listed))
  }
}

impl SecretsScreen {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.view.revision.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::SecretsSpacing::Body.role())
      .child(list::filters(ctx, &self.view, None))
      .child(load_error(
        ctx,
        "secrets-load-error",
        (
          &ctx.t("secrets.loadFailed"),
          &ctx.t_args("secrets.loadFailureDetail", [("reason", message_text(ctx, error))]),
        ),
        move || revision.update(|revision| *revision += 1),
      ))
      .into()
  }

  fn body(&self, ctx: &mut Ctx, (project, shown): (&ShownProject, SecretsView), listed: &ProjectSecrets) -> Element {
    let environment = shown.environment;
    let secret = shown
      .secret
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
    match (shown.panel, secret) {
      (SecretsPanel::Add, _) => {
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
      (SecretsPanel::Replace, Some(secret)) => {
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
            archive: shown.panel == SecretsPanel::Archive,
          },
        ))
      }
      _ => {}
    }
    body.into()
  }
}
