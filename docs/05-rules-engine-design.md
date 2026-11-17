# 05 — Rules engine design

## 1. Why

Fees, duties, taxes, deadlines and mandatory-clause lists change with each Finance Law and each
ministerial circular. The code must not need a release for a rate change, and an old calculation must
still reproduce the number it produced on the day it was signed.

## 2. Data shape

```
rule_set
  id, kind (fee|tax|deadline|checklist|succession), code, label_key,
  valid_from, valid_to, source, status, reviewed_by, reviewed_on, version_no

rule
  id, rule_set_id, code, label_key, kind (rate_tranche|flat|percentage|threshold|parameter),
  payload_json, priority, source, status
```

`payload_json` examples (values are **data**, never code):

```jsonc
// rate_tranche (codes DE-08-243 tranche 1..n)
{ "from": "0", "to": "10000000", "rate_num": 5, "rate_den": 100, "min": "0", "max": null }
// flat
{ "amount": "50000" }
// percentage
{ "rate_num": 250, "rate_den": 10000, "base": "act_value" }
// parameter
{ "key": "rounding", "value": "half_up" }
```

Amounts are centimes as decimal strings; rates are exact rationals. No `float` ever appears.

## 3. Evaluation

```
resolve(kind, code, on_date)      -> the single rule set whose [valid_from, valid_to) contains on_date
calculate(act_type, base_amount, on_date, items)
  -> for each item: pick matching rules by priority, apply, accumulate
  -> Calculation { lines[], total_state_levies, total_notary_fees, vat, grand_total,
                   rule_set_ids[], evaluated_at, rounding }
```

Rules applied in `priority` order; tranche rules are exclusive by `[from, to)`. A line records the
rule id it came from, so the printed cost statement can cite its legal basis and the calculation can
be replayed years later — the `calculation` row stores the rule set ids and the exact line amounts.

## 4. Guards

| Guard | Behaviour |
|---|---|
| Overlapping effective dates for the same `(kind, code)` | Import rejected, diff shown |
| Gap in effective dates around today | Warning banner + Home alert |
| `valid_to` in the past | "Tariff not updated" on Home and on every calculator |
| Rule with `status = [VERIFY]` in production mode | Calculation allowed but flagged; override justification required and audited for a non-notary user |
| Rules package not signed / signature invalid | Refused, logged |
| Rule import | Four-eyes: one user proposes, a notary approves; both actions audited |
| Deleting a rule set | Impossible; close it with `valid_to` |

## 5. Signed rule packages

A package is a JSON file `{ manifest, rules[], signature }` where `signature` is an Ed25519
signature over the canonical JSON of `{manifest, rules}`. The office key pair is generated at
initialisation; a package is accepted only if signed by a key in the trusted set (the office's own
key or a chamber-distributed key). Import writes a `rule_update_package` row and requires approval
before activation. Phase 3 implements the transport; Phase 1 implements the schema and the
signature check helper.

## 6. Testing

* Golden cases per formula: hand-computed expectations, one test per rule code, with the rule set
  pinned by id and date so the test fails loudly if a rate changes.
* Property tests: for random base amounts, tranche accumulation is monotonic and no line exceeds its
  `max`; total = sum of lines (exact integer arithmetic).
* Date tests: a calculation on the last day of a rule set and the first day of the next set resolve to
  the correct sets; a gap date produces the warning.
* Reproducibility: serialise a calculation, reload it, recompute from stored rule set ids → identical
  amounts.

## 7. What the notary sees

Cost statement (décompte / état de frais), printed in the user's language:

```
Détail des frais — Dossier 2026/0142                    [rule set: DE-08-243 v3, 2025-01-01 → ]
Honoraires (tranches)                                   85 000,00 DA
TVA 19% sur honoraires                                  16 150,00 DA
Droits d'enregistrement (2,5% [VERIFY])                125 000,00 DA
Taxe de publicité foncière (1% [VERIFY])                50 000,00 DA
Droit fixe                                                 500,00 DA
Total dû par le client                                 276 650,00 DA
dont levées pour l'État                                175 500,00 DA
dont honoraires + TVA                                  101 150,00 DA
```

Unverified rates print with a visible `[VERIFY]` marker and a footnote naming who must confirm them.
That is deliberate: the software must never look more certain than the law allows.
