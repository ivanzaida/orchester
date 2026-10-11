//! New secret and Replace value, in the inline inspector (YGy5i, yTCmk).
mod fields;
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::secret::{Environment, Secret, SecretKind, SecretValue};
use orchester_workspace::{
  activity::operations,
  secrets::{
    data::{ProjectSecrets, ReplacementPermission},
    form::{NameCheck, check_name},
  },
};
use zeroize::Zeroize;

use super::{
  inspector,
  state::{EnvironmentChoice, KindChoice, Panel, View},
  work,
};
use crate::{
  project::ShownProject,
  screens::connections::widen::{WidenConfirm, WidenProps},
  services::Services,
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub listed: ProjectSecrets,
  pub secret: Option<Secret>,
  pub view: View,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.listed == other.listed && self.secret == other.secret
  }
}
impl DevtoolsInspectable for Props {}
pub(super) struct SecretForm {
  action: work::Action,
  name: Signal<String>,
  kind: Signal<KindChoice>,
  environment: Signal<EnvironmentChoice>,
  /// Deliberately not a Ctx signal: no value in DevTools or its history.
  value: Signal<String>,
  asking: Signal<bool>,
}
impl Component for SecretForm {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let form = Self {
      action: work::action(ctx),
      name: ctx.signal(String::new()),
      kind: ctx.signal(KindChoice(
        props.secret.as_ref().map_or(SecretKind::ApiKey, Secret::kind),
      )),
      environment: ctx.signal(EnvironmentChoice(Some(props.secret.as_ref().map_or_else(
        || props.view.environment.get().0.unwrap_or(Environment::NEW_SECRET),
        Secret::environment,
      )))),
      value: Signal::new(String::new()),
      asking: ctx.signal(false),
    };
    let action = form.action.clone();
    let value = form.value.clone();
    let environment = form.environment.clone();
    let asking = form.asking.clone();
    ctx.watch(&action.state(), move |state| {
      match state.data.filter(|_| state.is_fulfilled()) {
        Some(work::Outcome::Saved(id)) => {
          value.update(Zeroize::zeroize);
          props
            .view
            .reveal(id, environment.get().0.expect("the form selects an environment"));
          props.view.revision.update(|revision| *revision += 1);
        }
        Some(work::Outcome::NeedsPerson) => {
          asking.set(true);
          props.view.revision.update(|revision| *revision += 1);
        }
        None => {}
      }
    });
    form
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let action = self.action.clone();
    let state = action.state().get();
    let check = check_name(
      &props.listed.secrets,
      &self.name.get(),
      self
        .environment
        .get()
        .0
        .expect("the form always selects an environment"),
    );
    let header = match &props.secret {
      Some(secret) => inspector::header(ctx, secret),
      None => fields::new_header(ctx, &props, self.cancel(&props), state.is_pending()),
    };
    let body = self.fields(ctx, &props, &check, state.is_pending());
    let footer = self.footer(ctx, &props, &check, &action);
    inspector::frame(header, body, footer)
  }
}
impl Drop for SecretForm {
  fn drop(&mut self) {
    self.value.update(Zeroize::zeroize);
  }
}
impl SecretForm {
  fn cancel(&self, props: &Props) -> Arc<dyn Fn() + Send + Sync> {
    let (value, panel) = (self.value.clone(), props.view.panel.clone());
    Arc::new(move || {
      value.update(Zeroize::zeroize);
      panel.set(Panel::Inspect);
    })
  }
  fn save(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    check: &NameCheck,
    action: &work::Action,
    permission: ReplacementPermission,
  ) -> Option<Arc<dyn Fn() + Send + Sync>> {
    if action.state().get().is_pending() || self.value.with(|value| value.trim().is_empty()) {
      return None;
    }
    let (write, spec) = match &props.secret {
      Some(secret) => (
        work::Write::Replace(secret.id(), permission),
        operations::secret_replace(&props.project.open, secret.name().as_str()),
      ),
      None => {
        let new = check.new_secret(
          self.kind.get().0,
          self.environment.get().0.expect("the form selects an environment"),
        )?;
        let spec = operations::secret_add(&props.project.open, new.name.as_str());
        (work::Write::Add(new), spec)
      }
    };
    let (action, value, vault, repo, asking) = (
      action.clone(),
      self.value.clone(),
      Services::of(ctx).vault,
      props.project.open.repo_root.clone(),
      self.asking.clone(),
    );
    Some(Arc::new(move || {
      let value = value.with(|value| SecretValue::new(value.clone()));
      let Ok(value) = value else {
        return;
      };
      if action.state().get().is_pending() {
        return;
      }
      let write = write.clone();
      asking.set(false);
      action.run(work::Start {
        repo: repo.clone(),
        vault: vault.clone(),
        write,
        value: Some(value),
        spec: spec.clone(),
      });
    }))
  }
  fn question(&self, ctx: &mut Ctx, save: Arc<dyn Fn() + Send + Sync>) -> Element {
    let asking = self.asking.clone();
    ctx.mount_keyed::<WidenConfirm>(
      "secret-confirm-replacement",
      WidenProps {
        id: "secret-confirm-replacement",
        card: false,
        automation: crate::mcp::input_gate::Guarded::WideningQuestion,
        title: ctx.t("secrets.rotation.question"),
        reasons: Arc::from([ctx.t("secrets.rotation.reason")]),
        confirm: ctx.t("secrets.replace"),
        keep: ctx.t("secrets.rotation.keep"),
        on_confirm: save,
        on_keep: Arc::new(move || asking.set(false)),
      },
    )
  }
}
