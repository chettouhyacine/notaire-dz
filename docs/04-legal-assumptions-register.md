# 04 — Legal Assumptions Register

**Status legend:** `[VERIFY]` = unconfirmed, must not be relied on in production · `[DRAFT]` = taken
from a secondary source, awaiting a primary text · `[CONFIRMED]` = checked against the official text
by a named reviewer.

**Nothing in this file is legal advice.** The application stores these items as data (see
`05-rules-engine-design.md`); editing a row here is a rules change that requires a notary's
signature in the app (Admin → Legal rules).

| # | Rule / assumption | Where it lives | Source | Status | Who must confirm |
|---|---|---|---|---|---|
| L1 | Notary fees are computed by tranches per Executive Decree 08-243 of 3 Aug 2008 | `rule_set` kind `fee`, code `NOTARY_TARIFF` | Decree 08-243 | [DRAFT] | Notary + Chambre nationale |
| L2 | Exact tranche boundaries and percentages of the notary tariff | `rule` rows of `NOTARY_TARIFF` | Decree 08-243 text (not yet obtained) | [VERIFY] | Notary, against the Journal officiel |
| L3 | Registration duty rate on real-estate sales (sources report 2.5% vs 5%) | `rule_set` kind `tax`, code `REG_DUTY_SALE` | Finance Law, current year | [VERIFY] — **sources conflict** | Notary + tax office (recette) |
| L4 | Land-publication tax ≈ 1% | `rule_set` `PUBLICATION_TAX` | Secondary sources | [VERIFY] | Conservation foncière |
| L5 | VAT at 19% applies to notary fees (not to State duties) | `rule_set` `VAT` | Finance Law | [VERIFY] | Notary / accountant |
| L6 | Fixed duty of 500 DA and its exact legal basis | `rule_set` `FIXED_DUTY` | Secondary sources | [VERIFY] | Notary |
| L7 | Capital-gains tax on real-estate resale: rate, base, exemptions, who collects | `rule_set` `CAPITAL_GAINS` | Finance Law | [VERIFY] | Notary + tax office |
| L8 | Rounding of computed amounts (assumed HALF_UP away from zero) | rule parameter `rounding` | Practice | [VERIFY] | Notary / accountant |
| L9 | Since 1 Jan 2025 real-estate payments must pass through the banking system; notaries are the enforcement point | rule `PAYMENT_CHANNEL_REAL_ESTATE` + hard guardrail flag | Announced reform, secondary sources | [VERIFY] — scope and fines | Notary, against the official circular |
| L10 | Fine amounts per breach of the cash ban | rule `CASH_BAN_PENALTY` (informational only) | Press reports | [VERIFY] | Notary |
| L11 | Notaries are designated non-financial professionals under Law 05-01 as amended: CDD, beneficial owner, PEP, STR to CTRF | module `aml` (Phase 4) | Law 05-01 | [DRAFT] | Notary / compliance officer |
| L12 | AML record-retention period | rule `AML_RETENTION_YEARS` (default placeholder, disabled) | Not confirmed | [VERIFY] — **do not default silently** | Notary + CTRF guidance |
| L13 | CTRF report format and submission channel | adapter `ctrf_export` (Phase 4) | Not published to us | [VERIFY] | Notary / CTRF |
| L14 | Whether the legal original (minute) may be electronic yet | adapter `esignature` (inert) | Law 15-04 + announced reforms | [VERIFY] | Notary + chamber |
| L15 | Language of the act: Arabic required, translation handling | template metadata `language_policy` | Legal practice | [VERIFY] | Notary |
| L16 | Which registers are legally mandatory (répertoire, powers, deposits, others) | `register` rows + `REGISTER_REQUIRED` rule | Practice / ministerial circulars | [VERIFY] | Notary + chamber |
| L17 | Retention period for acts and case files | rule `ACT_RETENTION_YEARS` | Notarial practice | [VERIFY] | Notary + chamber |
| L18 | Territorial competence tied to the court of the office's location; exact article | `office.jurisdiction` + validation note | Law 06-02 | [VERIFY] — article number | Lawyer |
| L19 | Compulsory civil-liability insurance (reported art. 43) | office settings `insurance_*` | Secondary source | [VERIFY] | Notary / insurer |
| L20 | Disciplinary sanctions and referral bodies (reported art. 54 onward) | informational only, not automated | Law 06-02 | [VERIFY] | Lawyer |
| L21 | Heirship rules: fixed shares, residuary heirs, exclusion, `'awl`/`radd`, bequest limit 1/3, grandchildren's substitute share | succession rule set (`SUCCESSION_RULES`, Phase 4) | Code de la famille, Book on successions | [VERIFY] — **expert-validated suite required before release** | Qualified legal expert (mufti/notary) |
| L22 | Law 18-07 duties: declarations, ANPDP formalities, cross-border transfer limits | DPIA + processing register | Law 18-07 as amended | [VERIFY] | Lawyer / DPO |
| L23 | Deed of notoriety/possession procedure (reported Law 07-02) | template + checklist | Secondary sources | [VERIFY] | Notary |
| L24 | Witnesses required per act type | template metadata `witnesses_required` | Practice | [VERIFY] | Notary |
| L25 | Current wilaya/daïra/commune list (administrative division has changed) | `territorial_unit` table | Official division | [VERIFY] — import required | Office / Interior Ministry list |
| L26 | Algerian public holidays incl. moving religious dates | `holiday` rows per year | Official calendar per year | [VERIFY] — yearly | Office |
| L27 | Weekend is Friday–Saturday | `office_settings.weekend` | Stated in brief | [DRAFT] | Office |
| L28 | Which taxes the notary collects vs merely verifies | `rule.applies_to` = collect/verify | Practice | [VERIFY] | Notary |

## Rules of the road

1. No rate, deadline, article number or mandatory clause appears in source code. A grep of `core/`
   and `app/` for a percentage or a decree number must return nothing but tests and seed data
   marked `[VERIFY]`.
2. Every rule row carries `source`, `status`, `reviewed_by`, `reviewed_on`.
3. A rule set cannot be activated while any of its rows is `[VERIFY]` **and** the office has
   switched on "production mode"; in that case the calculation screen shows the warning banner and
   the notary must record an override justification, which is audited.
4. The Finance-Law watcher warns when today's date is beyond `valid_to` of the active set, and Home
   shows "Tariff not updated for the current period".
5. When a rate changes, the old set is closed with `valid_to`, never edited — past calculations must
   stay reproducible.
