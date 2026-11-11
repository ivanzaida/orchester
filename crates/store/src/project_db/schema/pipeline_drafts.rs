//! Pipelines have no drafts (D-032, D-033): a pipeline is stored only as a published version. Every pipeline draft is
//! deleted, with no notice: a local pipeline's, a person's own in a team project, and an admin's local draft of a team
//! pipeline, with the marks of drafts automation saved and the money step budgets read from drafts. A pipeline with no
//! version at all is deleted too, with what names it: its default mark, its archive mark, where it was imported from,
//! and the pipeline of a task that chose it, which then has none.
//!
//! Left as they are: a shared task's `pipeline_slug` naming a deleted pipeline by its resource id (it reads as missing
//! then, `ProjectDb::missing_pipelines`, and an edit of the task sends no pipeline change, so the team's task keeps its
//! pipeline), a task's creation snapshot (immutable history), an instruction or skill scoped to a deleted
//! pipeline inside a published version (immutable; the scope then applies nowhere), and an Orchestrator proposal to
//! publish one (refused as changed when applied). A pipeline with no version never ran, so no run names it.

pub(super) const MIGRATION: &str = r"
CREATE TEMP TABLE dropped_pipelines AS
  SELECT id FROM resources WHERE kind = 'pipeline'
    AND id NOT IN (SELECT resource_id FROM resource_versions);

UPDATE tasks SET pipeline_id = NULL, pipeline_version = NULL
  WHERE pipeline_id IN (SELECT id FROM dropped_pipelines);
DELETE FROM project_default_pipeline WHERE pipeline_id IN (SELECT id FROM dropped_pipelines);
DELETE FROM automation_drafts WHERE resource_id IN (SELECT id FROM resources WHERE kind = 'pipeline');
DELETE FROM money_step_budgets_before_tokens WHERE owner = 'draft'
  AND owner_id IN (SELECT id FROM resources WHERE kind = 'pipeline');
DELETE FROM resource_drafts WHERE resource_id IN (SELECT id FROM resources WHERE kind = 'pipeline');
DELETE FROM resource_imports WHERE resource_id IN (SELECT id FROM dropped_pipelines);
DELETE FROM repository_imports WHERE resource_id IN (SELECT id FROM dropped_pipelines);
DELETE FROM removed_resources WHERE resource_id IN (SELECT id FROM dropped_pipelines);
DELETE FROM resources WHERE id IN (SELECT id FROM dropped_pipelines);

DROP TABLE dropped_pipelines;
";
