# 03 — Threat model (STRIDE)

Assets: client identity and family data, act contents, client funds, registers and numbering,
accounting, AML reports, master key and backups, the office's professional reputation.

Adversaries: opportunistic thief, curious or malicious employee, ransomware operator, malware on a
workstation, remote attacker on the office LAN, an insider exfiltrating data, a supplier of a
tampered update, and the inspecting authority (legitimate, but scope-limited).

## 1. Threat table

| # | Threat (STRIDE) | Vector | Mitigation in this design | Phase | Residual risk |
|---|---|---|---|---|---|
| T1 | Spoofing | Stolen laptop, weak or shared password | Argon2id, complexity + breach list, lockout/back-off, MFA (TOTP/FIDO2) mandatory for notary/admin, auto-lock, short idle timeout | 1 | Password written on a note; mitigated by MFA and by encrypted disk |
| T2 | Spoofing | Colleague using an unlocked session | Idle lock, re-auth for sensitive actions, per-user session tokens bound to the process | 1 | Shoulder-surfing during an active session |
| T3 | Tampering | Editing registers/acts/accounting in the DB | Finalised rows immutable by trigger + no UPDATE grant in code, corrections are new linked rows, HMAC-chained audit, daily head hash countersigned | 1 | An attacker with the master key and the binary can rewrite both data and chain; detected only against the countersigned head hashes |
| T4 | Tampering | Forged entry in the audit log | HMAC-SHA256 chain over `(seq, prev_hash, payload)`; verification tool; head hash printed daily | 1 | Same as above, bounded by the paper trail |
| T5 | Repudiation | "I never finalised that act" | Signed-off event with actor, device and timestamp in the chain; printed head hash | 1/3 | No hardware signature until the e-signature adapter is enabled |
| T6 | Information disclosure | Stolen disk or backup tape | Field-level AES-256-GCM envelope, per-file document keys, encrypted backups with an independent key | 1 | Structural metadata (counts, dates, amounts) visible — documented in D-1 |
| T7 | Information disclosure | Curious employee (clerk, accountant, IT admin) | RBAC + per-case ACL enforced in core; IT admin cannot decrypt case content; break-glass with justification and alert | 1 | A user with an authorised role can still read what the role allows — that is the design |
| T8 | Information disclosure | USB copy, print, screenshot, clipboard | Export/print permission + watermark (user, time), clipboard auto-clear, print log, volume anomaly alert | 1/5 | Camera photograph of a screen |
| T9 | Information disclosure | Network sniffing on office LAN | Default: no listener. LAN mode: mutual TLS, pinned certs, per-user tokens, no internet exposure | 5 | Compromised endpoint on the LAN |
| T10 | Denial of service | Ransomware encrypting DB, vault and backups | Offline/immutable backup copy, versioned backups, integrity verification, restore drill, documented RTO ≤ 4 h | 1/5 | Data loss since last backup (RPO ≤ 24 h) |
| T11 | Elevation of privilege | UI-only permission checks | Every command re-checks permission and ACL in Rust; tests assert the matrix per role | 1 | A core bug — covered by permission-matrix tests |
| T12 | Elevation of privilege | Malicious DOCX/PDF/TIFF parser input | Parsing isolated in a restricted subprocess, size/time limits, dependency audit, fuzzing | 2/5 | Zero-day in a third-party parser |
| T13 | Supply chain | Tampered dependency or update package | Lockfiles, `cargo audit`/`npm audit` in CI, SBOM, signed update packages with rollback, reproducible builds, no remote content | 5 | Compromised upstream maintainer |
| T14 | Social engineering | Caller asking a clerk to "read out" an act or change a bank account | Role limits, re-auth for bank-detail changes, four-eyes on critical changes, in-app warning banner on the payment screen | 1/3 | Human judgment |
| T15 | Legal compulsion / inspection | Authority demanding data | Read-only inspector role, scoped export, access log, no bulk silent export | 4 | Out-of-app copies |
| T16 | Privacy | Over-collection, indefinite retention | Data minimisation, retention schedule per record class, access log per client, DPIA | 2/4 | Regulatory interpretation of Law 18-07 `[VERIFY]` |

## 2. Controls required before Phase 1 can be called done

1. Master key never in memory longer than needed; zeroised on lock.
2. No plaintext secret or client data in logs, temp files, crash dumps or the UI.
3. Every sensitive command re-authenticates or checks freshness of the session.
4. Audit chain verifies end-to-end on startup and on demand; a broken chain blocks exports and warns
   the notary in plain language.
5. Backup restore drill executed at least once and recorded.

## 3. Explicitly out of scope for Phase 1

* Hardware token enrolment and Shamir key split (Phase 5).
* OS-level full-disk encryption enforcement (documented as a deployment requirement: BitLocker with
  TPM must be enabled on the office PC; the app cannot verify it reliably).
* Network mode hardening.
* Malware on the workstation with admin rights: no user-space application can defend against a
  kernel-level attacker; this is why backups are offline and the audit head is countersigned on
  paper.

## 4. Security acceptance tests

| Test | Expected |
|---|---|
| Open the SQLite file with a plain SQLite browser | Names, NINs, addresses unreadable |
| Flip one byte in any audit row | `audit_verify` fails and names the first broken sequence |
| Grant a clerk `act.finalise` in the UI only | Core refuses (command-level test) |
| Copy a case row and change its owner | ACL denies access |
| Delete a finalised act | DB rejects (immutability trigger) |
| Restore a backup into a scratch dir | Row counts and audit head match the source |
