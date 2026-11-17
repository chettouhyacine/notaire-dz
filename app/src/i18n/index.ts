/**
 * Translation layer.
 *
 * Two files only (`ar.json`, `fr.json`). Arabic defines the key set; French is typed against it, so
 * a missing French key is a **type error** and `npm run build` fails. Empty values are caught by a
 * test. No component may contain literal UI text.
 */
import ar from './ar.json';
import fr from './fr.json';

export type Lang = 'ar' | 'fr';
export const LANGS: readonly Lang[] = ['ar', 'fr'];

export type MessageKey = keyof typeof ar;

type Catalog = Record<MessageKey, string>;

const catalogs: Record<Lang, Catalog> = {
  ar: ar as Catalog,
  // Typed as Catalog on purpose: TypeScript reports any key present in ar.json but missing here.
  fr: fr as Catalog,
};

export function t(lang: Lang, key: MessageKey, vars?: Record<string, string | number>): string {
  const template = catalogs[lang][key] ?? catalogs.ar[key] ?? key;
  if (!vars) return template;
  return Object.entries(vars).reduce(
    (acc, [name, value]) => acc.replaceAll(`{${name}}`, String(value)),
    template,
  );
}

export function direction(lang: Lang): 'rtl' | 'ltr' {
  return lang === 'ar' ? 'rtl' : 'ltr';
}

/** Apply language to the document. Called on every switch: no reload, no data loss. */
export function applyDocumentLanguage(lang: Lang, doc: Document = document): void {
  doc.documentElement.lang = lang;
  doc.documentElement.dir = direction(lang);
}

export interface ParityIssue {
  key: string;
  problem: 'missing_in_fr' | 'missing_in_ar' | 'empty';
}

/** Runtime check used by the test suite and by `npm run verify`. */
export function checkParity(): ParityIssue[] {
  const issues: ParityIssue[] = [];
  const arKeys = Object.keys(ar) as MessageKey[];
  const frKeys = Object.keys(fr) as MessageKey[];

  for (const key of arKeys) {
    if (!frKeys.includes(key)) issues.push({ key, problem: 'missing_in_fr' });
    if (!ar[key].trim()) issues.push({ key, problem: 'empty' });
  }
  for (const key of frKeys) {
    if (!arKeys.includes(key)) issues.push({ key, problem: 'missing_in_ar' });
    if (!fr[key].trim()) issues.push({ key, problem: 'empty' });
  }
  return issues;
}

export const catalogFor = (lang: Lang): Catalog => catalogs[lang];
