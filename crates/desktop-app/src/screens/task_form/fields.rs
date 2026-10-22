//! The form's fields as signals the sections type into, read together as [`Values`]; the task's module beside
//! them, which is not one of the task's details: it is saved after them and is no part of a version's conflict.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_workspace::modules::ModuleChoice;

use super::{
  criteria::{Criterion, texts},
  plan::Pick,
  points::Points,
  values::Values,
};

#[derive(Clone)]
pub struct Fields {
  pub title: Signal<String>,
  pub brief: Signal<String>,
  pub criteria: Signal<Vec<Criterion>>,
  pub pipeline: Signal<Pick>,
  pub budget: Signal<String>,
  pub time_limit: Signal<String>,
  pub points: Signal<Points>,
  pub no_automatic_merge: Signal<bool>,
  pub module: Signal<ModuleChoice>,
}

impl Fields {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      title: ctx.signal(String::new()),
      brief: ctx.signal(String::new()),
      criteria: ctx.signal(Vec::new()),
      pipeline: ctx.signal(Pick(None)),
      budget: ctx.signal(String::new()),
      time_limit: ctx.signal(String::new()),
      points: ctx.signal(Points(None)),
      no_automatic_merge: ctx.signal(false),
      module: ctx.signal(ModuleChoice::default()),
    }
  }

  /// The fields' values; reading them subscribes the render to every field.
  pub fn values(&self) -> Values {
    Values {
      title: self.title.get(),
      brief: self.brief.get(),
      criteria: texts(&self.criteria.get()),
      pipeline: self.pipeline.get().0,
      budget: self.budget.get(),
      time_limit: self.time_limit.get(),
      points: self.points.get().0,
      no_automatic_merge: self.no_automatic_merge.get(),
    }
  }

  pub fn values_untracked(&self) -> Values {
    Values {
      title: self.title.get_untracked(),
      brief: self.brief.get_untracked(),
      criteria: self
        .criteria
        .get_untracked()
        .iter()
        .map(|row| row.text.get_untracked())
        .collect(),
      pipeline: self.pipeline.get_untracked().0,
      budget: self.budget.get_untracked(),
      time_limit: self.time_limit.get_untracked(),
      points: self.points.get_untracked().0,
      no_automatic_merge: self.no_automatic_merge.get_untracked(),
    }
  }

  pub fn fill(&self, values: &Values) {
    self.title.set(values.title.clone());
    self.brief.set(values.brief.clone());
    self.criteria.set(Criterion::rows(&values.criteria));
    self.pipeline.set(Pick(values.pipeline));
    self.budget.set(values.budget.clone());
    self.time_limit.set(values.time_limit.clone());
    self.points.set(Points(values.points));
    self.no_automatic_merge.set(values.no_automatic_merge);
  }
}
