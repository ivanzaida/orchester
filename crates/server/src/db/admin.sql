-- Schema 4, the server admin (docs/architecture/server.md, "The server admin"): browser sign-ins to the admin as
-- their own kind of device token, the server's name set by an admin, who linked the cloud, who revoked an invite.
ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'app' CHECK (kind IN ('app','admin_web'));
ALTER TABLE settings ADD COLUMN name TEXT;
ALTER TABLE cloud_link ADD COLUMN linked_by TEXT REFERENCES users(id);
ALTER TABLE invites ADD COLUMN revoked_by TEXT REFERENCES users(id);
CREATE INDEX invites_made_by ON invites(made_by, created_at);
UPDATE meta SET schema_version=4;
