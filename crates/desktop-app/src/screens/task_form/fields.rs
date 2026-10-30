//! The form's fields as signals the composer types into and its chips set, read together as [`Values`]; the task's
//! module and base branch beside them, which are not the task's details: they are saved after them and are no part of
//! a version's conflict.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::task::organization::Priority;
use orchester_workspace::{modules::ModuleChoice, tasks::base_branch::BasePick};

use super::{
  criteria::{Criterion, texts},
  delivers::DeliversPick,
  plan::Pick,
  points::Points,
  priority::PriorityPick,
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
  pub priority: Signal<PriorityPick>,
  pub delivers: Signal<DeliversPick>,
  pub module: Signal<ModuleChoice>,
  pub base_branch: Signal<BasePick>,
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
      priority: ctx.signal(PriorityPick(Priority::None)),
      delivers: ctx.signal(DeliversPick(None)),
      module: ctx.signal(ModuleChoice::default()),
      base_branch: ctx.signal(BasePick::default()),
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
      priority: self.priority.get().0,
      delivers: self.delivers.get().0,
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
      priority: self.priority.get_untracked().0,
      delivers: self.delivers.get_untracked().0,
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
    self.priority.set(PriorityPick(values.priority));
    self.delivers.set(DeliversPick(values.delivers));
  }
}
