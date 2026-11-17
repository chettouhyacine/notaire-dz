# 08 — Phased plan and status

## Phase 0 — Analysis ✅ delivered

Architecture (`01`), data model + ERD (`02`), threat model (`03`), legal assumptions register (`04`),
rules-engine design (`05`), UI wireframes + i18n/RTL rules (`06`), test strategy (`07`), this plan,
DPIA template (`09`).

## Phase 1 — Secure foundation ✅ delivered (this build)

| Item | Where |
|---|---|
| Portable toolchain, no global installs | `scripts/env.sh`, `.tools/` |
| Crypto envelope: Argon2id → master key → data keys, AES-256-GCM, XChaCha20-Poly1305 for documents | `core/src/crypto.rs` |
| Blind indexes with Arabic normalisation | `core/src/crypto.rs` |
| Audit hash chain (HMAC-SHA256), verify + head hash | `core/src/audit.rs` |
| RBAC matrix with per-case ACL, four-eyes, notary-only actions | `core/src/rbac.rs` |
| Integer money and exact rational rates | `core/src/money.rs` |
| Versioned rules engine with effective dates and reproducibility | `core/src/rules.rs` |
| SQLite storage, checksummed migrations, immutability triggers | `core/src/storage.rs`, `core/src/migrations/` |
| Encrypted backup create/verify/restore | `core/src/backup.rs` |
| Auth with lockout, re-auth for sensitive ops, idle timeout | `core/src/auth.rs` |
| React shell: first-run language, login, 8-entry sidebar, Home cards, AR/FR instant switch with RTL | `app/` |
| Tests: crypto KAT, audit tamper, RBAC matrix, money, rules golden cases, storage plaintext scan, migration, backup roundtrip | `core/tests/`, `app/src/**/*.test.ts` |
| Desktop shell (Tauri 2) with setup, login, audit head, encrypted backup commands | `desktop/src-tauri/` |
| Demo script | `docs/10-phase1-demo.md` |

Verified locally on 2026-11-15 (project root `F:\NotaireDZ`):

* `cargo test` in `core/` — **56 unit + 6 integration tests pass** (crypto KAT, audit tamper/
truncation/reorder, RBAC matrix, money rounding, rules golden cases and reproducibility, plaintext
scan of the database files, migration checksum abort, audit immutability triggers, backup roundtrip,
login lockout).
* `npm run verify` in `app/` — typecheck passes, **15 UI tests pass**, production build succeeds.
* `cargo build` in `desktop/src-tauri/` — desktop binary builds (one benign linker warning about
rsrc manifests on the GNU toolchain).
* **Not verified:** the GUI was not clicked through, and no test runs against a real office
passphrase workflow in the packaged app. Use `docs/10-phase1-demo.md` for that.

Known Phase 1 limitations, stated rather than hidden:

1. Single office passphrase authenticates one user; per-user key wrapping (`user_key_wrap`, created
   in migration 0002) is wired in Phase 2.
2. Database-wide encryption uses the documented envelope design (D-1), not SQLCipher.
3. No installer, no code signing, no MFA token yet — Phase 5.

Not in Phase 1 (by design): parties, cases, documents, registers, acts, accounting, AML, succession,
LAN, installer.

## Phase 2 — Core practice ◑ core delivered, agenda outstanding

| Item | State | Where |
|---|---|---|
| Parties: persons and companies, Arabic **and** Latin name forms stored side by side | done | `core/src/party.rs` |
| Duplicate detection: HMAC token match + Jaro-Winkler ranking, never automatic | done | `party::duplicate_candidates` |
| Identity document fields (NIN, birth data) encrypted; NIN blind-indexed | done | `party_person`, migration 0003 |
| Case files with 11-stage lifecycle, recorded stage events, ACL-filtered listing | done | `core/src/casefile.rs` |
| Checklists instantiated from a versioned template carrying its `[VERIFY]` status | done | `casefile::attach_checklist` |
| Document vault: per-document key, AES envelope, SHA-256 verified on every read, legal hold | done | `core/src/vault.rs` |
| OCR behind an `OcrEngine` interface; offline Tesseract adapter; unavailable is a first-class state | done | `core/src/ocr.rs` |
| Registers: gap-free numbering only after signature, hash-chained entries, closed registers refuse numbers | done | `core/src/registers.rs` |
| Minutes: physical location, loan/return log, delete refused by trigger | done | `registers::store_minute` |
| Copies: numbered per act and kind, watermarked, immutable | done | `registers::issue_copy` |
| Clients screen (create, search, dual-script results) and Cases screen (create, list, stage badge) | done | `app/src/screens/Clients.tsx`, `Cases.tsx` |
| Agenda with conflict detection | **not started** — moved to Phase 3 start | — |
| Document upload UI and checklist interaction UI | **not started** — needs the Tauri dialog plugin; core is ready and tested | — |

Verified locally: `cargo test` — **88 unit + 7 integration tests pass**, including an end-to-end
workflow (client → confidential case → encrypted document → OCR text → signature → register number →
issued copy → audit chain verified → no plaintext on disk). `npm run verify` — 20 UI tests, typecheck
and production build pass. Desktop shell builds with the new party/case commands.

Open items carried forward:

1. Tesseract is **not installed on this machine**, so OCR reports “engine unavailable” rather than
   producing text. Install `tesseract` with `ara` + `fra` language data before demonstrating OCR.
2. Checklist templates are seeded per call in tests; the notary-signed template library is Phase 3.
3. Documents can be stored and read through the core but the UI has no file picker yet.
4. `[VERIFY]` items L2–L22 still apply to every rate and checklist that ships.

## Phase 3 — Drafting and money

Template engine (variable clauses, repeatable blocks, clause library), the ten most common acts,
DOCX/PDF rendering with correct Arabic shaping and embedded fonts, amounts in words (AR/FR), fee and
tax engine wired to signed rule packages, client-funds accounting, cost statements.

## Phase 4 — Compliance

AML workflow (KYC, risk matrix, PEP, STR to CTRF with restricted visibility, no tipping-off),
succession calculator with exact fractions and a legal-expert-validated case suite, inspection mode,
reports.

## Phase 5 — Hardening and release

Pen-test fixes, performance work, signed installer, signed offline updates with rollback, LAN mode
with mutual TLS, manuals (AR/FR), training material, legacy import wizard, Shamir recovery split,
optional SQLCipher migration (see D-1).

## Open [VERIFY] items carried into Phase 2+

L2–L12 (all rates, thresholds, financial thresholds), L14 (e-signature status), L16 (mandatory
registers), L21 (succession rules), L22 (Law 18-07 duties), L25 (territorial list), L26 (holidays).
Full table in `04-legal-assumptions-register.md`.

## What still needs a human before any production use

1. A notary must review and sign off every rule set, template and checklist (the app enforces
   `reviewed_by`/`reviewed_on` before "production" is selectable).
2. A lawyer must confirm L18–L22 against the official texts.
3. A qualified expert must validate the succession case suite before Phase 4 ships.
4. The office must enable BitLocker and verify the restore drill.
