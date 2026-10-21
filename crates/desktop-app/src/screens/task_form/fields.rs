//! The form's fields as signals the sections type into, read together as [`Values`].

use lurq::{app::ctx::Ctx, core::Signal};

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
  }
}
