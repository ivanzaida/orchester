-- Schema 6, repeated invites (docs/architecture/server.md, "Invites"): an account keeps the HMAC of the verified email
-- its cloud identity last came with, never the email itself, so an invite by email to someone who has an account here
-- or is a member already is refused. Accounts from before it get one at their next cloud join or admin sign-in.
ALTER TABLE users ADD COLUMN email_tag BLOB;
CREATE INDEX users_email_tag ON users(email_tag) WHERE email_tag IS NOT NULL;
CREATE INDEX invites_open_email ON invites(email) WHERE email IS NOT NULL;
UPDATE meta SET schema_version=6;
