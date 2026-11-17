# 01 — Architecture

## 1. Shape

```
┌──────────────────────────────────────────────────────────────┐
│ Tauri 2 desktop shell (Rust, WebView2)                       │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ React + TypeScript UI (app/)                           │  │
│  │  • i18n ar/fr, RTL/LTR, no text in code                │  │
│  │  • calls only the typed command API below              │  │
│  └────────────────────────────────────────────────────────┘  │
│                    ▲  narrow typed IPC (serde)               │
│  ┌─────────────────┴──────────────────────────────────────┐  │
│  │ notaire-core (Rust)                                    │  │
│  │  auth · rbac · crypto · audit · rules · money ·        │  │
│  │  storage (SQLite) · backup · (later) acts, accounting,  │  │
│  │  aml, succession                                       │  │
│  └────────────────────────────────────────────────────────┘  │
│                    │                                         │
│              SQLite file + encrypted document vault (disk)   │
└──────────────────────────────────────────────────────────────┘
```

No HTTP listener in single-workstation mode. The UI cannot reach the filesystem or the database
directly: every operation goes through a command that re-checks permissions server-side.

## 2. Layering rules

1. **Domain never imports UI.** `notaire-core` has no Tauri dependency; it is a plain library with
   its own tests, so it can be reused by the Phase 5 office-server process.
2. **Adapters at the edge.** Tax/registration, land registry, commercial register, civil status,
   CTRF, AADL/bank forms, e-signature and OCR sit behind traits. Phase 1 ships a `ManualAdapter`
   (printable/importable files). No endpoint is ever hard-coded.
3. **Rules are data.** Fee, tax, deadline, checklist and template content live in versioned tables
   with `valid_from` / `valid_to` / `source` / `reviewed_by`. Code contains no rate, no article
   number and no deadline.
4. **Enforcement in the core.** A permission check that exists only in the UI is a bug.

## 3. Command API (Phase 1 surface)

| Command | Permission | Notes |
|---|---|---|
| `office_initialize` | — (first run only) | Creates office, first notary, master key. |
| `auth_login` / `auth_logout` | — | Argon2id verify, lockout/back-off, session token. |
| `auth_session` | — | Returns authenticated principal, roles, idle deadline. |
| `settings_get` / `settings_set` | `settings.manage` | Re-auth required for security settings. |
| `parties_search` (Phase 2) | `party.read` | Blind-index search on names. |
| `case_list` (Phase 2) | `case.read` + ACL | Filters per-case ACL. |
| `rules_active` | `rules.read` | Returns active rule sets and staleness warnings. |
| `rules_import_package` | `rules.write` + four-eyes | Signed package, dry-run diff, audit entry. |
| `audit_head` / `audit_verify` | `audit.read` | Head hash for daily countersignature. |
| `backup_create` / `backup_verify` / `backup_restore` | `backup.manage` | Re-auth; restore drills logged. |
| `export_*` | resource perm + `export.approve` | Watermarked, logged, optional approval. |

Every command returns `Result<T, AppError>` where `AppError` carries a translation key, never a raw
internal string, so both locales render the message.

## 4. Deployment modes

* **A — Single workstation (Phase 1).** One process, one database file, no sockets.
* **B — Office LAN (Phase 5).** The same `notaire-core` behind an office-server process: mutual TLS
  with pinned certificates, per-user tokens, rate limiting, file locking, no internet exposure and
  no default credentials.

## 5. Technology choices and trade-offs

| Choice | Why | Trade-off accepted |
|---|---|---|
| Tauri 2 + React + TS | Small binary, no Node in the shipped app, native WebView2, strict CSP possible. | Rust build complexity; WebView2 runtime required (present on Win 10/11). |
| Rust core | Memory safety for a parser- and crypto-heavy app. | Slower initial development than Node. |
| SQLite | Single-file, transactional, proven, no server, good FTS5. | Needs careful WAL/backup discipline. |
| No ORM | Parameterised SQL only, explicit and auditable. | More handwritten SQL. |
| Integer money (centimes) + rational rates | No floating point anywhere, exact reproduction of old calculations. | Explicit rounding policy needed, kept as a rule parameter. |

## 6. Deviation D-1 — database encryption (documented, deliberate)

**Requirement:** SQLCipher AES-256 or equivalent.

**What we ship in Phase 1:** application-layer envelope encryption.
* Every sensitive column (identity, address, contact, NIN, bank details, AML answers, free text) is
  stored as an AES-256-GCM ciphertext blob with a per-record data key.
* Data keys are wrapped by a master key derived with Argon2id (m=64 MiB, t=3, p=1) from the
  notary's passphrase and, in the shipped app, additionally protected by Windows DPAPI.
* Searchable fields get a **blind index**: HMAC-SHA256 of the normalised value under a dedicated
  index key, so a stolen database leaks no names and still supports exact-prefix lookup.
* `PRAGMA temp_store=MEMORY`, `journal_mode=WAL`; SQLite never writes decrypted content to disk.
* Non-sensitive structural columns (ids, timestamps, stage codes, amounts, rule versions) remain
  plaintext so that indexes and reports stay fast on a 4 GB machine.

**Why not SQLCipher right now:** the vendored OpenSSL build requires a full Perl toolchain that is
not present on the target build machine; shipping a crypto component that cannot be rebuilt
reproducibly is worse than a documented, testable alternative.

**Consequence, stated plainly:** the SQLite file's page layout and the set of *non-sensitive*
columns are visible to someone who steals the disk. Names, identities, NINs, bank data, AML answers
and document contents are not. Cross-referencing by row count/date is possible in principle.

**How to close the gap (Phase 5):** `storage` is behind a `Database` trait with a single open path.
Enabling `sqlcipher` is a feature flag plus a migration that copies plaintext pages through
`sqlcipher_export`. The field-level encryption stays on top, so nothing above the trait changes.

## 7. Backup topology (design)

* Continuous WAL archive + nightly full encrypted backup (AES-256-GCM, separate backup key).
* 3-2-1: local disk, rotating external drive, second office location.
* Recovery key split with Shamir secret sharing (3-of-5) between notary, associate and a trusted
  person — a Phase 5 item, with the split performed at initialisation.
* Automated integrity check and a scheduled test-restore into a scratch directory; failures raise a
  Home-screen alert. RPO ≤ 24 h, RTO ≤ 4 h.

## 8. Directory ↔ module mapping

| Concern | Location |
|---|---|
| Crypto, keys, blind index | `core/src/crypto.rs` |
| Audit chain | `core/src/audit.rs` |
| Roles/permissions | `core/src/rbac.rs` |
| Money and rounding | `core/src/money.rs` |
| Rules engine | `core/src/rules.rs` |
| SQLite + migrations | `core/src/storage.rs` |
| Backup/restore | `core/src/backup.rs` |
| Translations | `app/src/i18n/ar.json`, `fr.json` |
