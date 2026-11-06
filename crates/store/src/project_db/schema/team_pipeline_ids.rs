//! A team task names one of the team's pipelines by its resource id (`docs/architecture/server.md` row 80), no longer
//! by its slug; the team server turned the slugs it kept into ids without sending a change. The cached shared tasks
//! take the id the same way: the team pipeline this computer has of that slug, or none. The column keeps its name.
//! Task creation snapshots cannot change; a slug they recorded names no pipeline from now on.

pub(super) const MIGRATION: &str = r"
UPDATE tasks SET pipeline_slug = (
    SELECT resources.id FROM resources
    WHERE resources.kind = 'pipeline' AND resources.sharing = 'shared' AND resources.slug = tasks.pipeline_slug)
  WHERE sharing = 'shared' AND pipeline_slug IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM resources WHERE resources.id = tasks.pipeline_slug);
";
