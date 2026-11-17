-- Phase 2 schema: parties, cases, checklists, document vault, registers, minutes, copies.
-- Sensitive columns are ciphertext (`_enc`); searchable ones carry an HMAC blind index (`_bidx`).

-- ---------- parties ----------

CREATE TABLE party (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL CHECK (kind IN ('person', 'company')),
    -- Blind index over the normalised full name, used for duplicate detection and exact search.
    full_bidx     BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    created_by    TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_party_bidx ON party(full_bidx);

-- One row per script: an Algerian party legitimately has an Arabic and a Latin form of the same
-- name, and both must be stored and linkable rather than transliterated on the fly.
CREATE TABLE party_name (
    party_id      TEXT NOT NULL REFERENCES party(id),
    lang          TEXT NOT NULL CHECK (lang IN ('ar', 'fr')),
    surname_enc   BLOB NOT NULL,
    given_enc     BLOB NOT NULL,
    surname_bidx  BLOB NOT NULL,
    given_bidx    BLOB NOT NULL,
    full_bidx     BLOB NOT NULL,
    PRIMARY KEY (party_id, lang)
);

CREATE INDEX idx_party_name_surname ON party_name(surname_bidx);
CREATE INDEX idx_party_name_given ON party_name(given_bidx);
CREATE INDEX idx_party_name_full ON party_name(full_bidx);

CREATE TABLE party_person (
    party_id          TEXT PRIMARY KEY REFERENCES party(id),
    father_name_enc   BLOB,
    mother_name_enc   BLOB,
    birth_date        TEXT,
    birth_place_enc   BLOB,
    nationality_enc   BLOB,
    profession_enc    BLOB,
    marital_status    TEXT,
    nin_enc           BLOB,
    nin_bidx          BLOB
);

CREATE TABLE party_company (
    party_id       TEXT PRIMARY KEY REFERENCES party(id),
    legal_form     TEXT NOT NULL,
    rc_enc         BLOB,
    nif_enc        BLOB,
    nis_enc        BLOB,
    address_enc    BLOB,
    capital_cents  INTEGER,
    registry_valid_until TEXT
);

CREATE TABLE party_relation (
    id            TEXT PRIMARY KEY,
    from_party    TEXT NOT NULL REFERENCES party(id),
    to_party      TEXT NOT NULL REFERENCES party(id),
    kind          TEXT NOT NULL,
    note_enc      BLOB,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_relation_from ON party_relation(from_party);

CREATE TABLE identity_doc (
    id              TEXT PRIMARY KEY,
    party_id        TEXT NOT NULL REFERENCES party(id),
    doc_type        TEXT NOT NULL,
    number_enc      BLOB NOT NULL,
    number_bidx     BLOB NOT NULL,
    issued_on       TEXT,
    expires_on      TEXT,
    authority_enc   BLOB,
    verified_at_ms  INTEGER
);

CREATE INDEX idx_identity_party ON identity_doc(party_id);
CREATE INDEX idx_identity_expiry ON identity_doc(expires_on);

-- ---------- cases ----------

CREATE TABLE case_counter (
    year     INTEGER PRIMARY KEY,
    next_seq INTEGER NOT NULL
);

CREATE TABLE case_file (
    id            TEXT PRIMARY KEY,
    reference     TEXT NOT NULL UNIQUE,
    act_type      TEXT NOT NULL,
    stage         TEXT NOT NULL,
    owner_user_id TEXT NOT NULL,
    opened_at_ms  INTEGER NOT NULL,
    due_at_ms     INTEGER,
    confidential  INTEGER NOT NULL DEFAULT 0,
    notes_enc     BLOB,
    created_by    TEXT NOT NULL
);

CREATE INDEX idx_case_stage ON case_file(stage, due_at_ms);
CREATE INDEX idx_case_opened ON case_file(opened_at_ms);

CREATE TABLE case_party (
    case_id   TEXT NOT NULL REFERENCES case_file(id),
    party_id  TEXT NOT NULL REFERENCES party(id),
    role      TEXT NOT NULL,
    PRIMARY KEY (case_id, party_id, role)
);

CREATE TABLE case_stage_event (
    id            TEXT PRIMARY KEY,
    case_id       TEXT NOT NULL REFERENCES case_file(id),
    stage         TEXT NOT NULL,
    entered_at_ms INTEGER NOT NULL,
    actor_id      TEXT NOT NULL,
    note_enc      BLOB
);

CREATE INDEX idx_stage_event_case ON case_stage_event(case_id, entered_at_ms);

CREATE TABLE checklist_template (
    id         TEXT PRIMARY KEY,
    act_type   TEXT NOT NULL,
    version_no INTEGER NOT NULL,
    source     TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'verify',
    reviewed_by TEXT,
    reviewed_on TEXT
);

CREATE TABLE checklist_template_item (
    id           TEXT PRIMARY KEY,
    template_id  TEXT NOT NULL REFERENCES checklist_template(id),
    code         TEXT NOT NULL,
    label_key    TEXT NOT NULL,
    required     INTEGER NOT NULL DEFAULT 1,
    ordinal      INTEGER NOT NULL
);

CREATE TABLE checklist_instance (
    id           TEXT PRIMARY KEY,
    case_id      TEXT NOT NULL REFERENCES case_file(id),
    template_id  TEXT NOT NULL REFERENCES checklist_template(id),
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE checklist_item (
    id           TEXT PRIMARY KEY,
    instance_id  TEXT NOT NULL REFERENCES checklist_instance(id),
    code         TEXT NOT NULL,
    label_key    TEXT NOT NULL,
    required     INTEGER NOT NULL,
    status       TEXT NOT NULL,
    document_id  TEXT,
    expires_on   TEXT,
    note_enc     BLOB
);

CREATE INDEX idx_checklist_case ON checklist_instance(case_id);

-- ---------- document vault ----------

CREATE TABLE document (
    id              TEXT PRIMARY KEY,
    case_id         TEXT REFERENCES case_file(id),
    title_enc       BLOB NOT NULL,
    mime            TEXT NOT NULL,
    sha256          TEXT NOT NULL,
    byte_size       INTEGER NOT NULL,
    vault_name      TEXT NOT NULL,
    version_no      INTEGER NOT NULL DEFAULT 1,
    retention_class TEXT NOT NULL DEFAULT 'case',
    legal_hold      INTEGER NOT NULL DEFAULT 0,
    ocr_text_enc    BLOB,
    created_at_ms   INTEGER NOT NULL,
    created_by      TEXT NOT NULL
);

CREATE INDEX idx_document_case ON document(case_id);
CREATE INDEX idx_document_sha ON document(sha256);

CREATE TABLE document_version (
    id           TEXT PRIMARY KEY,
    document_id  TEXT NOT NULL REFERENCES document(id),
    version_no   INTEGER NOT NULL,
    sha256       TEXT NOT NULL,
    byte_size    INTEGER NOT NULL,
    vault_name   TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    created_by   TEXT NOT NULL
);

CREATE INDEX idx_document_version ON document_version(document_id, version_no);

CREATE TRIGGER document_no_delete BEFORE DELETE ON document
WHEN OLD.legal_hold = 1
BEGIN
    SELECT RAISE(ABORT, 'document is under legal hold');
END;

-- ---------- registers, minutes, copies ----------

CREATE TABLE register (
    id            TEXT PRIMARY KEY,
    code          TEXT NOT NULL,
    series_prefix TEXT NOT NULL,
    year          INTEGER NOT NULL,
    next_seq      INTEGER NOT NULL DEFAULT 1,
    office_id     TEXT NOT NULL,
    closed_at_ms  INTEGER,
    UNIQUE (office_id, code, year)
);

CREATE TABLE register_entry (
    id             TEXT PRIMARY KEY,
    register_id    TEXT NOT NULL REFERENCES register(id),
    seq            INTEGER NOT NULL,
    act_reference  TEXT NOT NULL,
    case_id        TEXT REFERENCES case_file(id),
    finalised_at_ms INTEGER NOT NULL,
    actor_id       TEXT NOT NULL,
    entry_hash     TEXT NOT NULL,
    UNIQUE (register_id, seq)
);

CREATE INDEX idx_register_entry_case ON register_entry(case_id);

-- A register entry is the legal trace of a number: it is never edited or removed.
CREATE TRIGGER register_entry_no_update BEFORE UPDATE ON register_entry
BEGIN
    SELECT RAISE(ABORT, 'a register entry is immutable');
END;

CREATE TRIGGER register_entry_no_delete BEFORE DELETE ON register_entry
BEGIN
    SELECT RAISE(ABORT, 'a register entry is immutable');
END;

CREATE TABLE minute (
    id            TEXT PRIMARY KEY,
    act_reference TEXT NOT NULL,
    case_id       TEXT REFERENCES case_file(id),
    room_enc      BLOB,
    shelf_enc     BLOB,
    box_enc       BLOB,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE minute_loan (
    id            TEXT PRIMARY KEY,
    minute_id     TEXT NOT NULL REFERENCES minute(id),
    borrower_enc  BLOB NOT NULL,
    reason_enc    BLOB,
    borrowed_at_ms INTEGER NOT NULL,
    returned_at_ms INTEGER,
    actor_id      TEXT NOT NULL
);

CREATE TRIGGER minute_no_delete BEFORE DELETE ON minute
BEGIN
    SELECT RAISE(ABORT, 'minutes are never destroyed');
END;

CREATE TABLE copy_issued (
    id            TEXT PRIMARY KEY,
    case_id       TEXT REFERENCES case_file(id),
    act_reference TEXT NOT NULL,
    copy_kind     TEXT NOT NULL CHECK (copy_kind IN ('simple', 'authentic', 'executory')),
    copy_number   INTEGER NOT NULL,
    recipient_enc BLOB NOT NULL,
    issued_at_ms  INTEGER NOT NULL,
    issued_by     TEXT NOT NULL,
    watermark     TEXT NOT NULL,
    UNIQUE (act_reference, copy_kind, copy_number)
);

CREATE TRIGGER copy_no_update BEFORE UPDATE ON copy_issued
BEGIN
    SELECT RAISE(ABORT, 'an issued copy is a legal record');
END;
