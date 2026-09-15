-- Adding an unvalidated FK takes a brief metadata lock. Fail quickly instead of queuing behind a
-- long transaction; the migration runner can retry without having caused a write outage.
SET LOCAL lock_timeout = '5s';

ALTER TABLE refresh_tokens
  ADD CONSTRAINT refresh_tokens_family_fk
    FOREIGN KEY (family_id) REFERENCES refresh_token_families(id) ON DELETE CASCADE
    NOT VALID;
