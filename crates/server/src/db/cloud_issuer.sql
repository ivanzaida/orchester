-- Schema 8, the cloud's move to another public URL (docs/cloud-hosting.md, "Moving the cloud"): the issuer the cached
-- key set named before a newer key set of the same root named another, whose tokens still verify.
ALTER TABLE cloud_link ADD COLUMN former_issuer TEXT;
UPDATE meta SET schema_version=8;
