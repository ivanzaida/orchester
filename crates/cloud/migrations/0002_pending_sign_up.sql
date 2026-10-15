-- Review 1, H1: a confirmation link carries the password and display name of the sign-up that sent it. Confirming
-- the link sets them on the account, so the person who can read the email chooses the password, not whoever signed
-- up first with that email. Reset links leave both empty.
ALTER TABLE email_links ADD COLUMN password_hash text;
ALTER TABLE email_links ADD COLUMN display_name text;
