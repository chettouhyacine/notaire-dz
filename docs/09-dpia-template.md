# 09 — DPIA template and data-processing register (Law 18-07 `[VERIFY]`)

> Template. Every field marked `[VERIFY]` must be confirmed with the ANPDP's published guidance and
> a lawyer before the office relies on it.

## 1. Processing register entry (one per processing activity)

| Field | Value |
|---|---|
| Activity | Client and case management for notarial acts |
| Controller | The notary office (name, address, registration) |
| Purpose | Drafting authentic acts, registers, fees, accounting, AML/CFT duties |
| Legal basis | Statutory notarial duties + contract with the client `[VERIFY]` |
| Data subjects | Clients, parties, heirs, representatives, staff |
| Categories of data | Identity, civil status, family links, property, financial, AML assessments, documents |
| Special categories | Possibly family/health data appearing in documents `[VERIFY]` |
| Recipients | None routine; State bodies where law requires (tax, land registry, CTRF) |
| Transfers outside Algeria | **None.** No cloud, no telemetry, no remote content. Exports are user-initiated files `[VERIFY Law 18-07 transfer rules]` |
| Retention | Per record class; act retention `[VERIFY]`; AML retention `[VERIFY]` |
| Security measures | Envelope encryption, RBAC + ACL, hash-chained audit, encrypted offline backups, device encryption |
| Data subject rights | Access/rectification handled by the notary; erasure limited by notarial retention duties `[VERIFY]` |

## 2. DPIA screening

| Question | Answer | Consequence |
|---|---|---|
| Systematic and extensive evaluation of persons? | No profiling; AML risk scoring only | Document the scoring matrix and its human review |
| Large-scale special-category data? | Possibly within documents | Restrict access, log every read |
| Systematic monitoring? | No | — |
| Data on vulnerable persons (minors, interdicted)? | Yes, in successions | Case ACL defaults to named users only |
| Innovative technology? | No AI in production; OCR offline with human confirmation | AI off by default; no external calls |

## 3. Risk register (privacy)

| Risk | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|
| Insider reading a famous client's file | Medium | High | Case ACL + audit of reads + break-glass justification | Low |
| Laptop theft | Medium | High | Field encryption + device encryption | Low |
| Over-retention | Medium | Medium | Retention timers and Home alerts | Medium `[VERIFY]` |
| Accidental export to the wrong recipient | Medium | High | Export approval, watermark, confirmation dialog | Low |
| Backup copy lost | Low | High | Backup encryption + Shamir-split recovery key | Low |

## 4. Data-minimisation decisions in the design

* Photo and signature specimen are optional fields, off by default.
* AML answers are stored only for parties actually assessed, under a separate ACL.
* Diagnostics export strips all client content and can be reviewed before sending.
* No analytics, no crash reporting to a third party, no update check that carries identifiers.

## 5. Open items

1. `[VERIFY]` Whether the office must file a prior declaration with the ANPDP and in what form.
2. `[VERIFY]` Maximum retention period for case files versus the notary's duty to keep minutes.
3. `[VERIFY]` Whether an inspection export counts as a transfer requiring a legal basis note.
