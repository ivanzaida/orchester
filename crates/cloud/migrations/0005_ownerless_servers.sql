-- Security review of the cloud-deploy branch, minor 1: deleting the account that linked a team server keeps the
-- server registered, with its members' joins and its invites, rather than unregistering it. The server has no owner
-- until the next link statement its key signs links it again (the newest statement owns it, migration 4).

ALTER TABLE servers ALTER COLUMN owner DROP NOT NULL;
ALTER TABLE servers DROP CONSTRAINT servers_owner_fkey;
ALTER TABLE servers ADD CONSTRAINT servers_owner_fkey FOREIGN KEY (owner) REFERENCES accounts (id) ON DELETE SET NULL;
