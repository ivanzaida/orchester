//! A publication's facts: the draft's name and versions, where an instruction or skill applies, the runs under way
//! that keep the version before, what the draft widens, what its kind tells of it and the text it changes. The draft
//! counts only while it is the one proposed (saved when the proposal was made); after publishing, the published
//! version stands in for it.

use orchester_core::{
  id::ResourceId,
  orchestrator::change::Publishing,
  resource::{
    AgentProfile, Draft, Instruction, InstructionScope, Pipeline, ResourceBody, Skill, Tool, ToolAction, Version,
  },
  run::{Run, StepExecutor},
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{Detail, Place, Publication, text};
use crate::{orchestrator::widening, tasks::detail::run_store_error};

/// The facts of publishing `resource`, of kind `of`, as version `version` from the draft saved at `saved`; `None`
/// when the resource is gone.
pub(super) fn read(
  db: &ProjectDb,
  (of, resource): (Publishing, ResourceId),
  (version, saved): (u32, Timestamp),
) -> Result<Option<Publication>, StoreError> {
  let Some(found) = db.resource(resource)? else {
    return Ok(None);
  };
  let mut publication = Publication {
    of,
    resource,
    name: found.name().as_str().to_owned(),
    version,
    before_published: None,
    saved,
    places: Vec::new(),
    keeping: keeping(db, (of, resource), version.saturating_sub(1))?,
    widens: Vec::new(),
    detail: None,
    diff: None,
  };
  match of {
    Publishing::Instruction => instruction(db, &mut publication)?,
    Publishing::Skill => skill(db, &mut publication)?,
    Publishing::Pipeline => pipeline(db, &mut publication)?,
    Publishing::Profile => profile(db, &mut publication)?,
    Publishing::Tool => tool(db, &mut publication)?,
  }
  Ok(Some(publication))
}

/// The version before the published one, and what is published: the draft while it is the one proposed (`true`),
/// else the published version.
struct Bodies<B> {
  before: Option<Version<B>>,
  after: Option<B>,
  proposed: bool,
}

fn bodies<B: ResourceBody + Clone>(db: &ProjectDb, publication: &Publication) -> Result<Bodies<B>, StoreError> {
  let versions = db.versions::<B>(publication.resource)?;
  let numbered = |number: u32| {
    versions
      .iter()
      .find(|version| version.number().get() == number)
      .cloned()
  };
  let draft = db
    .draft::<B>(publication.resource)?
    .filter(|draft| draft.updated_at() == publication.saved);
  let proposed = draft.is_some();
  let after = draft
    .as_ref()
    .map(Draft::body)
    .cloned()
    .or_else(|| numbered(publication.version).map(|version| version.body().clone()));
  Ok(Bodies {
    before: numbered(publication.version.saturating_sub(1)),
    after,
    proposed,
  })
}

/// Fills what all kinds share from `bodies`: when the version before was published, and what the draft widens.
fn shared<B>(db: &ProjectDb, publication: &mut Publication, bodies: &Bodies<B>) -> Result<(), StoreError> {
  publication.before_published = bodies.before.as_ref().map(Version::published_at);
  if bodies.proposed {
    publication.widens = widening::of_draft(db, publication.of, publication.resource)?;
  }
  Ok(())
}

fn instruction(db: &ProjectDb, publication: &mut Publication) -> Result<(), StoreError> {
  let bodies = bodies::<Instruction>(db, publication)?;
  shared(db, publication, &bodies)?;
  if let Some(after) = bodies.after.as_ref().or(bodies.before.as_ref().map(Version::body)) {
    publication.places = places(db, after.scope())?;
  }
  if let (true, Some(after)) = (bodies.proposed, &bodies.after) {
    let before = bodies
      .before
      .as_ref()
      .map_or("", |version| version.body().text().as_str());
    publication.diff = text::diff(before, after.text().as_str());
  }
  Ok(())
}

fn skill(db: &ProjectDb, publication: &mut Publication) -> Result<(), StoreError> {
  let bodies = bodies::<Skill>(db, publication)?;
  shared(db, publication, &bodies)?;
  if let Some(after) = &bodies.after {
    publication.places = places(db, after.scope())?;
    publication.detail = Some(Detail::Files {
      count: after.files().len(),
    });
    if bodies.proposed {
      let before = bodies.before.as_ref().map_or("", |version| version.body().body());
      publication.diff = text::diff(before, after.body());
    }
  }
  Ok(())
}

fn pipeline(db: &ProjectDb, publication: &mut Publication) -> Result<(), StoreError> {
  let bodies = bodies::<Pipeline>(db, publication)?;
  shared(db, publication, &bodies)?;
  if let Some(after) = &bodies.after {
    let before = bodies.before.as_ref().map_or(&[][..], |version| version.body().steps());
    let changed = after.steps().iter().filter(|step| !before.contains(step)).count()
      + before
        .iter()
        .filter(|step| !after.steps().iter().any(|after| after.key == step.key))
        .count();
    publication.detail = Some(Detail::Steps {
      count: after.steps().len(),
      changed,
    });
  }
  Ok(())
}

fn profile(db: &ProjectDb, publication: &mut Publication) -> Result<(), StoreError> {
  let bodies = bodies::<AgentProfile>(db, publication)?;
  shared(db, publication, &bodies)?;
  if let Some(after) = &bodies.after {
    publication.name = after.role().as_str().to_owned();
    let connection = db
      .connection(after.connection())?
      .map(|connection| connection.name().as_str().to_owned())
      .unwrap_or_default();
    publication.detail = Some(Detail::Model {
      connection,
      model: after.model().as_str().to_owned(),
    });
  }
  Ok(())
}

fn tool(db: &ProjectDb, publication: &mut Publication) -> Result<(), StoreError> {
  let bodies = bodies::<Tool>(db, publication)?;
  shared(db, publication, &bodies)?;
  if let Some(ToolAction::Command(command)) = bodies.after.as_ref().map(Tool::action) {
    publication.detail = Some(Detail::Command {
      program: command.program().as_str().to_owned(),
      args: command.args().iter().map(|arg| arg.as_str().to_owned()).collect(),
    });
  }
  Ok(())
}

/// Where `scope` applies, by the names people read now.
fn places(db: &ProjectDb, scope: &InstructionScope) -> Result<Vec<Place>, StoreError> {
  let pipeline_name = |pipeline: ResourceId| -> Result<String, StoreError> {
    Ok(
      db.resource(pipeline)?
        .map(|found| found.name().as_str().to_owned())
        .unwrap_or_default(),
    )
  };
  Ok(vec![match scope {
    InstructionScope::Project => Place::Project,
    InstructionScope::AgentRole { role } => Place::Role(role.as_str().to_owned()),
    InstructionScope::Pipeline { pipeline } => Place::Pipeline {
      pipeline: *pipeline,
      name: pipeline_name(*pipeline)?,
    },
    InstructionScope::Step { pipeline, step } => {
      let latest = db.latest_version::<Pipeline>(*pipeline)?;
      let named = latest
        .as_ref()
        .and_then(|version| version.body().steps().iter().find(|found| found.key == *step))
        .map_or_else(|| step.as_str().to_owned(), |found| found.name.as_str().to_owned());
      Place::Step {
        pipeline: *pipeline,
        name: pipeline_name(*pipeline)?,
        step: named,
      }
    }
  }])
}

/// The runs under way that run version `before` of `resource`, of kind `of`: they keep it until they end.
fn keeping(db: &ProjectDb, (of, resource): (Publishing, ResourceId), before: u32) -> Result<usize, StoreError> {
  if before == 0 {
    return Ok(0);
  }
  let pins = |run: &Run| {
    let snapshot = run.snapshot();
    let pinned = |pin: &orchester_core::run::PinnedVersion| pin.resource == resource && pin.number.get() == before;
    match of {
      Publishing::Pipeline => pinned(&snapshot.pipeline()),
      Publishing::Instruction => snapshot.instructions().iter().any(pinned),
      Publishing::Skill => snapshot.skills().iter().any(pinned),
      Publishing::Tool => snapshot.tools().iter().any(pinned),
      Publishing::Profile => snapshot
        .steps()
        .iter()
        .any(|step| matches!(step.executor(), StepExecutor::Agent { profile, .. } if pinned(profile))),
    }
  };
  let mut count = 0;
  for task in db.tasks()?.into_iter().chain(db.not_shared_tasks()?) {
    // A run that cannot be read is not counted: its task page says it cannot be read.
    for run in db
      .runs_of_task(task.id())
      .map_err(run_store_error)?
      .into_iter()
      .flatten()
    {
      count += usize::from(!run.status().is_terminal() && pins(&run));
    }
  }
  Ok(count)
}
