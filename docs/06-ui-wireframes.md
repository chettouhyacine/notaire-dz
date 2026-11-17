# 06 — UI: wireframes, i18n and RTL rules

## 1. Global frame (both directions)

Arabic (RTL): sidebar on the right, content flows right→left, icons mirrored except brand marks.
French (LTR): mirrored exactly. Only one layout implementation exists, using CSS logical properties
(`margin-inline-start`, `padding-inline-end`, `inset-inline-start`) and `dir` on `<html>`.

```
┌──────────────────────────────────────────────────────────────┐
│ [Logo] الجزائر — مكتب التوثيق   [ Search… ]   [AR|FR] [User]│  top bar, height 64
├───────────────┬──────────────────────────────────────────────┤
│ ⌂ الرئيسية    │  Screen title                         [+ New] │
│ ▤ الملفات     │  ────────────────────────────────────────    │
│ ◍ الزبائن     │                                              │
│ § العقود      │   content, max width 1100, one column        │
│ ⚖ المحاسبة    │                                              │
│ ⛨ الامتثال    │                                              │
│ ◷ المواعيد    │                                              │
│ ⚙ الإعدادات   │                                              │
└───────────────┴──────────────────────────────────────────────┘
```

Exactly 8 sidebar entries (Home, Cases, Clients, Acts, Accounting, Compliance, Calendar, Settings).
Sidebar width 220 px; content column ≥ 700 px; nothing scrolls horizontally at 1366×768.

## 2. First run

```
┌──────────────────────────────────────────┐
│            NotaireDZ                     │
│   Choose your language / اختر اللغة      │
│   ┌──────────────────────────────────┐   │
│   │            العربية               │   │  ← primary
│   └──────────────────────────────────┘   │
│   ┌──────────────────────────────────┐   │
│   │            Français              │   │
│   └──────────────────────────────────┘   │
└──────────────────────────────────────────┘
```

Choosing Arabic immediately flips `dir="rtl"`; no restart, no reload of data.

## 3. Home

Four cards + one primary action. No charts, no extra widgets.

```
┌──────────────┬──────────────┐
│ مواعيد اليوم │ ملفات تنتظر  │   ← today's appointments / cases waiting
│      3       │  إجراءً: 7   │
├──────────────┼──────────────┤
│ أرصدة غير    │ وثائق تنتهي  │
│ مسددة: 12    │ صلاحيتها: 4 │
└──────────────┴──────────────┘
        [  + ملف جديد  ]
```

## 4. Forms

One column, label above field, 12 px help text, inline validation under the field, plain-language
message that states the fix ("أدخل تاريخ الميلاد بصيغة يوم/شهر/سنة" / "Saisissez la date de naissance
au format JJ/MM/AAAA"). Autosave every 20 s and on blur; crash recovery reopens the same draft.
Advanced fields sit behind "خيارات إضافية / Plus d'options".

## 5. Wizard (act drafting)

```
[1 الأطراف] — [2 العقار] — [3 الثمن والأداء] — [4 الشروط] — [5 المراجعة] — [6 الطباعة]
  progress bar, current step bold, completed steps clickable, future steps locked
  primary button: التالي / Suivant     secondary: رجوع / Retour     discreet: خيارات إضافية
```

Step 5 shows blocking issues grouped as: missing mandatory field, expired document, missing banking
proof, checklist item outstanding, unverified rule. Each issue links to the exact field. The notary's
override requires a written justification and is audited.

## 6. Language switch

* Two translation files only: `app/src/i18n/ar.json`, `app/src/i18n/fr.json`.
* Keys are flat, dot-namespaced (`home.cards.appointments`). **No literal UI text in components.**
* `npm run verify` fails the build when: a key exists in one file only, a value is empty, or a
  component contains a hard-coded Arabic/Latin UI string (lint rule).
* The interface language and the act language are independent; the act language selector lives in
  the act wizard, not in the top bar.
* Numbers: Western digits by default, option for Arabic-Indic in Settings. Dates: Gregorian by
  default with an optional Hijri display line. Both are display-only; storage stays ISO.
* Names are stored twice (Arabic and Latin) in one party record and shown side by side where space
  allows, never concatenated into a single bidi-ambiguous string.

## 7. Typography and colour

| Token | Arabic UI | French UI |
|---|---|---|
| Font | Cairo (bundled) | Inter (bundled) |
| Body size | 16 px | 15 px |
| Line height | 1.75 | 1.55 |
| Accent | one colour: `#0f6b52` (deep green) | same |
| Surface | `#ffffff`, text `#111827`, border `#e5e7eb` | same |

Light theme default, dark mode optional (same accent, `#0b1220` surface). Contrast ≥ 4.5:1 for body
text, 3:1 for large text. Focus rings always visible.

## 8. Accessibility and keyboard

* Full keyboard operation: `Tab` order follows reading direction, `Enter` submits, `Esc` closes,
  `Ctrl+K` search, `Ctrl+N` new case.
* Focus never trapped inside a panel; dialogs return focus on close.
* Icons always paired with a text label (never icon-only for primary actions).
* Screen-reader labels come from the same translation files.

## 9. Acceptance tests (Section 14)

| # | Test | How it is checked |
|---|---|---|
| 1 | AR/FR switch updates all text, direction and number/date format instantly | Vitest + manual: toggle on Home, Cases, Settings |
| 2 | Clerk creates a client and opens a case in < 5 min without training | Timed usability rehearsal in Phase 2 demo |
| 3 | No horizontal scrolling, no truncated text, at 1366×768 | Layout check in both languages at that viewport |
| 4 | Mixed Arabic/Latin in one field renders correctly | Field-level check on party names (`محمد BENALI`) |
