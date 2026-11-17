# 07 — Test strategy

## 1. Layers

| Layer | Tool | Gate |
|---|---|---|
| Unit (Rust core) | `cargo test` | Every commit; must be green |
| Crypto known-answer tests | `cargo test` | Mandatory; fixed vectors, no random assertions |
| Property tests | `cargo test` (`proptest`) | Rules arithmetic, rounding, succession shares |
| Migration tests | `cargo test` | Empty DB, double-apply, fixture from previous release |
| Permission matrix | `cargo test` | One test per (role, command) pair, allow and deny |
| Audit chain | `cargo test` | Tamper, reorder, delete, replay |
| Backup/restore drill | `cargo test` + manual | Restore into scratch dir, compare row counts and head hash |
| UI typecheck + i18n parity | `npm run verify` | Build fails on missing/extra/empty translation key |
| UI behaviour | Vitest + Testing Library | Language switch, RTL attribute, form validation messages |
| RTL visual review | manual checklist | Phase 2+, per release |
| Performance | scripted | 100k cases / 1M documents, search < 1 s |
| Security | `cargo audit`, `npm audit`, fuzz parsers, pen-test checklist | Phase 5 |

## 2. Non-negotiable test rules

1. No test asserts on "it did not crash"; every test asserts a value, a denial, or a hash.
2. A failing security test is never fixed by relaxing the test.
3. Golden cases pin the rule set id and the evaluation date, so a tariff change breaks the test
   loudly instead of silently changing expected money.
4. Tests use fictional data only — never a real name, address, NIN or bank account.

## 3. Phase 1 test inventory (implemented)

| Test | Asserts |
|---|---|
| `crypto_kat` | AES-256-GCM and XChaCha vectors, Argon2id determinism with fixed salt |
| `field_encryption_roundtrip` | decrypt(encrypt(x)) == x; ciphertext differs per call (fresh nonce); wrong key fails |
| `blind_index_stable` | Same normalised Arabic name → same index; different spelling variant → same index (alef/ya/ta-marbuta) |
| `audit_chain_verify` | Append N events, verify ok; mutate one payload → verify fails at that seq |
| `audit_chain_truncate` | Removing the last event is detected via head-hash comparison |
| `rbac_matrix` | Clerk cannot finalise; IT admin cannot read case content; inspector is read-only; only notary approves closure |
| `money_rounding` | HALF_UP away from zero on exact halves, both signs; no float drift over 10 000 additions |
| `rules_tranche_golden` | Hand-computed fee for 3 sample amounts against a pinned rule set |
| `rules_effective_dates` | Correct set at boundary days; gap detected |
| `rules_overlap_rejected` | Overlapping sets refused |
| `storage_no_plaintext` | Writing a party with a known NIN leaves no plaintext occurrence in the DB file bytes |
| `migration_idempotent` | Checksum recorded; second apply refused; tampered checksum aborts |
| `backup_roundtrip` | Encrypted backup restores to identical row counts and audit head |
| `i18n_parity` | ar and fr have identical key sets, all non-empty |

## 4. Definition of done per phase

Source code + tests green + docs updated + demo script + list of open `[VERIFY]` items + a written
statement of what a notary or lawyer still has to confirm.
