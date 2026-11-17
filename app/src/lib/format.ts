/**
 * Display formatting. Storage is always ISO / integer centimes; only presentation varies by
 * language and by the user's digit preference. Western digits are the default.
 */
import type { Lang } from '../i18n';

export type DigitStyle = 'western' | 'arabicIndic';

const ARABIC_INDIC = ['٠', '١', '٢', '٣', '٤', '٥', '٦', '٧', '٨', '٩'];

export function toArabicIndic(input: string): string {
  return input.replace(/[0-9]/g, (d) => ARABIC_INDIC[Number(d)]);
}

/** 12 500 050 centimes → "125 000,50" (or with Arabic-Indic digits). */
export function formatMoney(
  centimes: number,
  _lang: Lang,
  digits: DigitStyle = 'western',
): string {
  const negative = centimes < 0;
  const abs = Math.abs(Math.trunc(centimes));
  const dinars = Math.floor(abs / 100);
  const cents = abs % 100;
  const grouped = dinars.toLocaleString('en-US').replaceAll(',', '\u00a0');
  const text = `${negative ? '-' : ''}${grouped},${String(cents).padStart(2, '0')}`;
  return digits === 'arabicIndic' ? toArabicIndic(text) : text;
}

/** "2026-03-09" → "09/03/2026". Never parses into a local Date to avoid timezone drift. */
export function formatDate(
  iso: string,
  _lang: Lang,
  digits: DigitStyle = 'western',
): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) return iso;
  const [, y, m, d] = match;
  const text = `${d}/${m}/${y}`;
  return digits === 'arabicIndic' ? toArabicIndic(text) : text;
}

/** Section 14: no screen may need horizontal scrolling at 1366×768. */
export const MIN_VIEWPORT_WIDTH = 1366;
