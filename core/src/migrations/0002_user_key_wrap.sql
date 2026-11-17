-- Phase 2 hook, created now so the schema never needs a destructive migration to add it.
-- Each user's passphrase-derived key wraps the office master key, so a clerk can unlock the office
-- without ever holding the office key itself.
CREATE TABLE user_key_wrap (
    user_id       TEXT PRIMARY KEY REFERENCES user(id),
    salt_hex      TEXT NOT NULL,
    wrapped_key   BLOB NOT NULL,
    kdf_m_cost    INTEGER NOT NULL,
    kdf_t_cost    INTEGER NOT NULL,
    kdf_p_cost    INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
);
