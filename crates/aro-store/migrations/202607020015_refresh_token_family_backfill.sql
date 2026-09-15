-- no-transaction
-- This file intentionally contains exactly one top-level CALL. SQLx therefore sends one simple
-- query outside an explicit transaction, allowing the procedure to commit each bounded batch.
CALL aro_backfill_refresh_token_families(1000);
