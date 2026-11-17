-- Recovery escrow.
--
-- The office master key is sealed under a one-time, high-entropy recovery code that setup shows to
-- the notary. It is the only way back into an office whose passphrase is forgotten: the passphrase
-- derives the master key, so without either the passphrase or this escrow the data stays
-- unreadable — by design, not as a bug.
--
-- Only the sealed key is stored; the code itself is never written anywhere, and losing both means
-- losing the office. The row is replaced whenever a new code is issued.
CREATE TABLE recovery_escrow (
    id             TEXT PRIMARY KEY,
    wrapped_master BLOB NOT NULL,
    created_at_ms  INTEGER NOT NULL
);

-- The master key wrapped under a passphrase-derived key lives in `user_key_wrap` (migration 0002),
-- which already carries its own salt and Argon2id parameters. A passphrase reset replaces that row
-- and keeps the master key unchanged, so existing records and documents still decrypt.
