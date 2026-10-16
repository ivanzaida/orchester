-- N1 security re-review 2: a team server keeps the person who linked it when their account is deleted, so the caps on
-- one account (five linked servers, 100 invites a day from them) hold for the person past the account: signing up
-- again with the same email gives none back. `last_owner` is the person's key, an HMAC of the account's email under a
-- key only the cloud holds (`crate::people`), never the email; a new link statement clears it with the new owner.
-- Servers ownerless before this migration have none: each counts on its own, as before.

ALTER TABLE servers ADD COLUMN last_owner bytea;
CREATE INDEX servers_last_owner ON servers (last_owner) WHERE owner IS NULL;
