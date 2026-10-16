-- N1 security review 1 (docs/cloud-api.md, "Accepting an invite"): an invite keeps the invitee's answer instead of
-- leaving the table. An accepted invite is hidden from `GET /invites` and still issues tokens that accept it to the
-- account that accepted it, until it expires or the server revokes it, so a join that failed after the token can be
-- tried again. A declined one issues none. Either way the row stays until the invite expires, so delivering the same
-- signed invite again changes nothing.

ALTER TABLE invites ADD COLUMN answer text CHECK (answer IN ('accepted', 'declined'));
ALTER TABLE invites ADD COLUMN answered_by uuid;
ALTER TABLE invites ADD COLUMN answered_at timestamptz;
ALTER TABLE invites ADD CONSTRAINT invites_answered CHECK ((answer IS NULL) = (answered_by IS NULL AND answered_at IS NULL));
