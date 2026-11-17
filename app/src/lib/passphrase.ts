/**
 * Passphrase rules, mirrored from the core.
 *
 * The core (`core/src/auth.rs::check_password_policy`) is authoritative and re-checks every
 * passphrase. This module exists only so the setup screen can show *why* a phrase is refused as
 * the user types, instead of after a round trip. Keep the two in step: same minimum, same classes,
 * same common list.
 */
export const PASSPHRASE_MIN_LENGTH = 12;

/** Mirrors `COMMON_PASSWORDS` in the core. */
const COMMON_PASSPHRASES: readonly string[] = [
  'password1234',
  'motdepasse123',
  '123456789012',
  'azertyuiop123',
  'qwertyuiop123',
  'administrateur',
  'notairenotaire',
  'aaaaaaaaaaaa',
  '000000000000',
  'letmein12345',
];

export interface PassphraseChecks {
  /** Long enough. */
  length: boolean;
  /**
   * Two of: letters (any script), digits, symbols — a space does not count as a symbol — or a long
   * phrase of at least {@link LONG_PASSPHRASE_LENGTH} characters, which word-based phrases need.
   */
  mixed: boolean;
  /** Not one character repeated. */
  notRepeated: boolean;
  /** Not in the small known-common list. */
  notListed: boolean;
}

/** Beyond this length a letters-only phrase is accepted; entropy comes from the length. */
export const LONG_PASSPHRASE_LENGTH = 20;

export function passphraseChecks(value: string): PassphraseChecks {
  const chars = [...value];
  const hasLetter = chars.some((c) => /\p{L}/u.test(c));
  const hasDigit = chars.some((c) => /\p{Nd}/u.test(c));
  const hasSymbol = chars.some((c) => /\p{S}|\p{P}/u.test(c));
  const classes = [hasLetter, hasDigit, hasSymbol].filter(Boolean).length;
  const repeated = chars.length > 0 && chars.every((c) => c === chars[0]);
  return {
    length: chars.length >= PASSPHRASE_MIN_LENGTH,
    mixed: classes >= 2 || (hasLetter && chars.length >= LONG_PASSPHRASE_LENGTH),
    notRepeated: !repeated,
    notListed: !COMMON_PASSPHRASES.includes(value.trim().toLowerCase()),
  };
}

export function isPassphraseAcceptable(value: string): boolean {
  const checks = passphraseChecks(value);
  return checks.length && checks.mixed && checks.notRepeated && checks.notListed;
}

export type PassphraseIssue = 'setup.tooShort' | 'setup.weak' | 'setup.common' | null;

/**
 * The first unmet rule, or `null` when the phrase is acceptable. The order mirrors
 * `check_password_policy` in the core so the message shown matches the one that would come back.
 */
export function passphraseIssue(value: string): PassphraseIssue {
  const checks = passphraseChecks(value);
  if (!checks.length) return 'setup.tooShort';
  if (!checks.notRepeated) return 'setup.common';
  if (!checks.mixed) return 'setup.weak';
  if (!checks.notListed) return 'setup.common';
  return null;
}

const WORDS: readonly string[] = [
  'nour',
  'kitab',
  'barid',
  'zitoun',
  'qalam',
  'salam',
  'amtar',
  'tariq',
  'bahr',
  'jabal',
  'warda',
  'qamar',
  'najm',
  'tamr',
  'zayt',
  'safsaf',
  'hobz',
  'dar',
  'bab',
  'sahla',
  'karmous',
  'wahat',
];

/**
 * A passphrase the notary can type and remember: four short words, a number, a symbol.
 * Randomness comes from `crypto.getRandomValues` when available, so suggestions differ per click.
 */
export function suggestPassphrase(random: () => number = randomIndex): string {
  const pick = (): string => WORDS[Math.floor(random() * WORDS.length)] ?? 'nour';
  const digits = 10 + Math.floor(random() * 90);
  const symbol = ['!', '-', '#', '*'][Math.floor(random() * 4)] ?? '!';
  return `${pick()}-${pick()}-${pick()}-${pick()}${digits}${symbol}`;
}

function randomIndex(): number {
  try {
    const buffer = new Uint32Array(1);
    crypto.getRandomValues(buffer);
    return buffer[0]! / 2 ** 32;
  } catch {
    return Math.random();
  }
}
