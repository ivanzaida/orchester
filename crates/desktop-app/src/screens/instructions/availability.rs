//! What the chosen preview (r8Ipz) says about an instruction's variables, read once by the editor and
//! handed to its fields, notes and panels as values, so no render writes another component's signals.
use std::{ops::Deref, sync::Arc};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureState},
};
use orchester_core::resource::Variable;
use orchester_workspace::{
  background,
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    preview::{PreviewCatalog, PreviewChoice},
    publication::{RetainedRun, retained_runs},
  },
  messages::{self, Message},
  tasks::outside::Outside,
};

use crate::project::ShownProject;

/// The catalog as read, shared: the editor and its panels compare it by pointer first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog(Arc<PreviewCatalog>);
impl DevtoolsInspectable for Catalog {}
impl Deref for Catalog {
  type Target = PreviewCatalog;

  fn deref(&self) -> &PreviewCatalog {
    &self.0
  }
}

pub type CatalogState = FutureState<Catalog, Message>;

/// The role and step the preview is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewContext {
  pub role: String,
  pub step: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Availability {
  /// Variables without a value in the preview; all of them while nothing can be previewed.
  pub unavailable: Vec<Variable>,
  /// Active runs that keep the instruction's published version.
  pub retained: Vec<RetainedRun>,
  /// `None` while nothing can be previewed.
  pub context: Option<PreviewContext>,
}

impl Availability {
  /// From `catalog` as last read; a read that failed gives nothing, its error is shown by the panels.
  pub fn of(ctx: &Ctx, catalog: &CatalogState, choice: &PreviewChoice, entry: Option<&InstructionEntry>) -> Self {
    let catalog = catalog.data.as_deref().filter(|_| !catalog.is_rejected());
    let retained = catalog
      .zip(entry)
      .map_or_else(Vec::new, |(catalog, entry)| retained_runs(catalog, entry.id));
    let Some(resolved) = catalog.and_then(|catalog| catalog.resolve(choice).ok()) else {
      return Self {
        unavailable: Variable::ALL.to_vec(),
        retained,
        context: None,
      };
    };
    let step = ctx.t_args(
      "instructions.stepLabel",
      [
        ("number", format!("{:02}", resolved.step_ordinal())),
        ("name", resolved.step.name.to_string()),
      ],
    );
    Self {
      unavailable: Variable::ALL
        .into_iter()
        .filter(|variable| resolved.value(*variable).is_none())
        .collect(),
      retained,
      context: Some(PreviewContext {
        role: resolved.role().to_owned(),
        step: step.to_string(),
      }),
    }
  }
}

/// The preview catalog, read again when the instructions or the project's tasks change.
pub fn catalog(ctx: &mut Ctx, project: &ShownProject, data: &ProjectInstructions) -> CatalogState {
  let changes = project.feed.outside.get();
  let root = project.open.repo_root.clone();
  let hold = crate::services::Services::of(ctx).hold;
  ctx
    .future(
      (
        root,
        data.clone(),
        changes.of(Outside::Tasks),
        changes.of(Outside::Instructions),
      ),
      move |(root, ..)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || PreviewCatalog::load_in(&root))
            .await?
            .map(|catalog| Catalog(Arc::new(catalog)))
            .map_err(|error| messages::store_error(&error))
        }
      },
    )
    .state()
    .get()
}
