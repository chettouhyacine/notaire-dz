# 10 — Phase 1 demo script

Everything below uses **fictional** data. Never enter a real client's details in a demo.

## A. Run the tests (5 minutes)

```bash
source scripts/env.sh
cargo test --manifest-path core/Cargo.toml      # 55 unit + 6 integration tests
cd app && npm run verify                        # typecheck + 15 UI tests + production build
```

Expected: `test result: ok` for both suites and `built in …` from Vite with exit code 0.

## B. Review the interface (browser preview, no office data)

```bash
cd app && npm run dev     # http://127.0.0.1:5173
```

1. **First run.** One screen, two choices. Pick **العربية** → layout flips to right-to-left
   immediately, sidebar on the right.
2. **Login.** Preview account: `notaire` / `Correct-Horse-9`. A wrong password shows a plain message
   and keeps you on the screen. (In the browser there is no database, so this preview cannot reach
   real data — the desktop build is the real thing.)
3. **Home.** Four cards and one primary action *ملف جديد*. No other widgets.
4. **Language switch.** Press **Français** in the top bar: every label, the direction, the sidebar
   side and the number format change instantly, without a reload and without losing your place.
5. **Settings → الأمان والتدقيق.** The audit panel reports that it is desktop-only in the browser.
6. **Navigation.** Each of the eight entries opens; the not-yet-built modules say plainly that they
   arrive in phase 2 instead of pretending a button works.
7. **Keyboard.** Tab through the sidebar and the form; focus rings are always visible.

## C. Desktop shell (real encryption, real audit chain)

```bash
source scripts/env.sh
cd desktop/src-tauri && cargo build             # produces target/debug/notairedz-desktop.exe
```

On first launch:

1. **Setup** asks for the office name, the notary's name, a username and a passphrase
   (≥ 12 characters, mixing letters and digits). The passphrase derives the office key with
   Argon2id — there is no recovery without it, which is the point.
2. The app creates `%LOCALAPPDATA%\NotaireDZ\office.sqlite` and `office.salt`.
3. **Settings → الأمان والتدقيق** shows the audit head hash and the event count: this is the value
   the notary prints and countersigns daily.
4. Type the passphrase and press **إنشاء نسخة احتياطية مشفرة** — a date-stamped backup appears in
   `%LOCALAPPDATA%\NotaireDZ\backups\`, verified by checksum before it is reported as done.
5. Close and reopen the app, sign in again: the audit chain must still verify (it is checked on
   every login, before any data is shown).

## D. Prove the security properties yourself

| Claim | How to check |
|---|---|
| Client data is not readable on disk | Open `office.sqlite` in any SQLite browser: `party`-style data does not exist yet, but ciphertext for the office name and the wrapped keys are visible, and no plaintext name appears |
| Audit log cannot be edited | In a SQLite browser, run `UPDATE audit_event SET detail='x'` → *audit log is append-only* |
| Finalised values cannot be edited | `UPDATE calculation SET total_cents=1` → refused by trigger |
| A wrong passphrase opens nothing | Sign in with the wrong passphrase: decryption fails before any data is read |
| A tampered backup is refused | Flip a byte in a `.nzdzbak` file → verify reports a checksum failure |
| Rule sets are never deleted | `DELETE FROM rule_set` → refused; rules are closed with `valid_to` |

## D2. Phase 2 additions (clients, cases, vault, registers)

In the desktop app after signing in:

1. **الزبائن / Clients** → *زبون جديد / Nouveau client*. Enter `BENZAKOUR` / `Mohamed` (Latin) and,
   optionally, `بن زاكور` / `محمد` (Arabic). Save. The record appears in the list with both scripts.
2. Create a second client with the same Latin name: the core's duplicate detection reports a match
   before the record is committed (see `party::duplicate_candidates`), so the same person is not
   filed twice.
3. **الملفات / Dossiers** → *ملف جديد / Nouveau dossier*. Choose *بيع عقاري / Vente immobilière*, set a
   due date, tick *ملف سري / Dossier confidentiel*, create. The reference is allocated gap-free
   (`2026/0001`) and the stage badge reads *الاستقبال / Accueil*.
4. Tick the confidentiality box and sign in as a clerk who is not on the case ACL: the case is
   absent from their list. That filter runs in Rust, not in the screen.

The rest of Phase 2 is exercised by the automated suite rather than by clicking, because the UI for
it is not built yet:

```bash
source scripts/env.sh
cargo test --manifest-path core/Cargo.toml --test phase2_workflow   # end-to-end
```

That test walks a client through a confidential case, an encrypted document, OCR text, signature,
a register number, an issued copy, then proves the audit chain verifies and that neither the database
nor the vault contains the party's name or NIN in plaintext.

## E. What this demo does **not** show (honest list)

* No document upload, checklist-ticking, register or copy screens: the core supports them and they
  are tested, but only Clients and Cases have screens so far.
* No agenda, acts, accounting or AML screens: phases 3–4.
* OCR needs `tesseract` installed with `ara` and `fra` data; it is not installed on this machine, so
  the engine honestly reports itself unavailable.
* No OCR, no PDF/DOCX rendering, no fee calculator screen — the rules engine is tested in the core,
  the UI for it arrives with Phase 3.
* Multi-user sign-in with per-user key wrapping (a clerk unlocking the office without holding the
  office key) is Phase 2; Phase 1 deliberately ships a single-office passphrase and says so.
* The GUI was compiled but not clicked through by the assistant: the checks above are the ones that
  were actually executed (`cargo test`, `npm run verify`, both green), plus this script for a human.
