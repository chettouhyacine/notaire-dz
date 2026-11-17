-- Phase 1 schema: office, users, roles, sessions, audit, rules, calculations.
-- Tables for parties/cases/acts/accounting arrive in their own migrations (Phase 2+); nothing is
-- created empty "for later" so a partial schema can never be mistaken for a complete one.

-- schema_migration is created by the migration runner before any migration is applied.

CREATE TABLE office (
    id             TEXT PRIMARY KEY,
    name_enc       BLOB NOT NULL,
    jurisdiction_enc BLOB,
    created_at_ms  INTEGER NOT NULL
);

CREATE TABLE office_settings (
    key           TEXT PRIMARY KEY,
    value         TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE role (
    code      TEXT PRIMARY KEY,
    label_key TEXT NOT NULL
);

CREATE TABLE permission (
    code      TEXT PRIMARY KEY,
    label_key TEXT NOT NULL
);

CREATE TABLE role_permission (
    role_code       TEXT NOT NULL REFERENCES role(code),
    permission_code TEXT NOT NULL REFERENCES permission(code),
    PRIMARY KEY (role_code, permission_code)
);

CREATE TABLE user (
    id                   TEXT PRIMARY KEY,
    username             TEXT NOT NULL UNIQUE,
    display_name_enc     BLOB,
    password_hash        TEXT NOT NULL,
    roles_json           TEXT NOT NULL,
    active               INTEGER NOT NULL DEFAULT 1,
    failed_attempts      INTEGER NOT NULL DEFAULT 0,
    locked_until_ms      INTEGER,
    created_at_ms        INTEGER NOT NULL,
    must_change_password INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE session (
    token_hash     TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES user(id),
    created_at_ms  INTEGER NOT NULL,
    last_seen_ms   INTEGER NOT NULL,
    expires_at_ms  INTEGER NOT NULL,
    reauth_until_ms INTEGER
);

CREATE INDEX idx_session_user ON session(user_id);

CREATE TABLE case_acl (
    case_id       TEXT NOT NULL,
    user_id       TEXT NOT NULL,
    granted_at_ms INTEGER NOT NULL,
    PRIMARY KEY (case_id, user_id)
);

-- Append-only audit log. The triggers are the last line of defence for T3/T4 (docs/03).
CREATE TABLE audit_event (
    seq            INTEGER PRIMARY KEY,
    occurred_at_ms INTEGER NOT NULL,
    actor_id       TEXT NOT NULL,
    action         TEXT NOT NULL,
    subject        TEXT NOT NULL,
    detail         TEXT NOT NULL,
    prev_hash      BLOB NOT NULL,
    hash           BLOB NOT NULL
);

CREATE INDEX idx_audit_occurred ON audit_event(occurred_at_ms);
CREATE INDEX idx_audit_actor ON audit_event(actor_id);

CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_event
BEGIN
    SELECT RAISE(ABORT, 'audit log is append-only');
END;

CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_event
BEGIN
    SELECT RAISE(ABORT, 'audit log is append-only');
END;

-- Rules: versioned, dated, sourced, reviewed. Closed with valid_to, never deleted.
CREATE TABLE rule_set (
    id           TEXT PRIMARY KEY,
    code         TEXT NOT NULL,
    kind         TEXT NOT NULL,
    version_no   INTEGER NOT NULL,
    valid_from   TEXT NOT NULL,
    valid_to     TEXT,
    source       TEXT NOT NULL,
    status       TEXT NOT NULL,
    reviewed_by  TEXT,
    reviewed_on  TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_rule_set_code ON rule_set(code);

CREATE TRIGGER rule_set_no_delete BEFORE DELETE ON rule_set
BEGIN
    SELECT RAISE(ABORT, 'rule sets are closed, never deleted');
END;

CREATE TABLE rule (
    id           TEXT PRIMARY KEY,
    rule_set_id  TEXT NOT NULL REFERENCES rule_set(id),
    code         TEXT NOT NULL,
    kind         TEXT NOT NULL,
    label_key    TEXT NOT NULL,
    priority     INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    status       TEXT NOT NULL,
    applies_to   TEXT NOT NULL
);

CREATE INDEX idx_rule_set ON rule(rule_set_id, priority);

CREATE TABLE calculation (
    id                 TEXT PRIMARY KEY,
    code               TEXT NOT NULL,
    evaluated_on       TEXT NOT NULL,
    rule_set_ids_json  TEXT NOT NULL,
    lines_json         TEXT NOT NULL,
    total_cents        INTEGER NOT NULL,
    state_levies_cents INTEGER NOT NULL,
    notary_fees_cents  INTEGER NOT NULL,
    rounding           TEXT NOT NULL,
    created_at_ms      INTEGER NOT NULL,
    created_by         TEXT
);

CREATE TRIGGER calculation_no_update BEFORE UPDATE ON calculation
BEGIN
    SELECT RAISE(ABORT, 'a stored calculation is a receipt and cannot be edited');
END;

CREATE TABLE app_setting (
    key           TEXT PRIMARY KEY,
    value         TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
