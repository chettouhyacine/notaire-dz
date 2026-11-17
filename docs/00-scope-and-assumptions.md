# 00 — Scope, assumptions, and blocking questions

## 1. Scope in ten lines

1. Offline-first Windows desktop application for one Algerian notary office; no internet needed.
2. Bilingual interface, Arabic primary (RTL) and French secondary (LTR), switchable instantly.
3. Party and client records (natural and legal persons), with Arabic + Latin name forms.
4. Case files with lifecycle stages, checklists, expiry tracking and activity timeline.
5. Drafting of authentic acts from versioned templates, Arabic by default, reviewable before signature.
6. Registers, minutes and copy issuance with gap-free numbering generated only at finalisation.
7. Fees, duties and taxes computed from dated, editable rule tables — never hard-coded — with reproducible receipts.
8. Double-entry office accounting plus a strictly separated client-funds ledger.
9. AML/CFT: KYC, risk scoring, suspicious-transaction workflow with restricted visibility.
10. Bank-grade protection: encryption at rest, RBAC + per-case ACLs, hash-chained audit, encrypted backups, secrecy-preserving exports.

Out of scope unless asked: cloud SaaS, client portal, mobile app, AI legal conclusions, unapproved
automatic submissions to State systems.

## 2. Assumptions (stated, not asked)

| # | Assumption | Impact if wrong |
|---|---|---|
| A1 | Phase 1 targets a **single workstation**, single notary account; LAN mode stays a Phase 5 deployment variant of the same core. | Schema and IPC unchanged; only transport and locking work added. |
| A2 | The legal original (minute) remains **paper, wet-signed**; the app produces drafts, registers and copies. | E-signature adapter slot exists but is inert. |
| A3 | Windows 10/11 x64, 1366×768 minimum, HDD, 4 GB RAM. | Lower-spec tuning in Phase 5. |
| A4 | No official State connectors are available to the office yet; all integrations are file-based/manual adapters. | Connector work swaps an adapter implementation. |
| A5 | Legacy data exists in Word/Excel and will be migrated in Phase 5 by an import wizard, not by direct DB writes. | Migration tooling moves earlier. |
| A6 | Amounts are in Algerian dinar, stored as integer **centimes**; rates as exact rationals; rounding is HALF_UP away from zero `[VERIFY]`. | Rounding policy is a rule-table parameter, not code. |
| A7 | The office has one or more notaries but Phase 1 seeds exactly one; roles/permissions already model the rest. | No schema change. |
| A8 | Professional secrecy outranks convenience: a blocked action stays blocked unless a notary records a justification. | None. |

## 3. Blocking questions (max 5)

The five questions below are the only ones that can change *structure*. Work proceeds on the
assumptions above until they are answered.

| # | Question | Why it blocks | Working assumption |
|---|---|---|---|
| Q1 | Single notary or SCP / bureaux groupés (multiple titular notaries sharing client funds)? | Changes ownership of registers, numbering series and accounting entities. | Single notary now; SCP modelled by `office_id` + per-notary numbering series. |
| Q2 | How many workstations, and is a shared office server acceptable? | Decides Phase 5 LAN work and backup topology. | One workstation; LAN later. |
| Q3 | What legacy data must be migrated (volume, formats, registers already numbered)? | Numbering continuity is legally sensitive; gaps must be preserved, not invented. | Excel/Word, imported in Phase 5 with a reconciliation report. |
| Q4 | Which Windows versions and is there a TPM/smart card on the notary's PC? | Determines whether FIDO2/PIV and DPAPI machine-scope are usable. | Win 10/11 x64; software keys + DPAPI now, token optional. |
| Q5 | Does the office already hold any official connector, portal account, or CTRF reporting format? | Determines whether an adapter is file-based or a live connector. | File-based adapters only. |

## 4. Success criteria traced to phases

| Criterion | Verified in |
|---|---|
| Clerk opens a case, drafts an Arabic sale deed, computes charges, prints a draft in < 15 min | Phase 3 (demo script + timed rehearsal) |
| No data leaves the office without explicit export | Phase 1 (no network listener, audit of every export) |
| Inspection can be passed from the app alone | Phase 4 (inspection mode + register/accounting exports) |
| Runs on 4 GB RAM / HDD, no internet | Phase 5 (performance + offline acceptance run) |
