import { describe, expect, it } from 'vitest';
import { isPassphraseAcceptable, passphraseChecks, passphraseIssue, suggestPassphrase } from './passphrase';

describe('passphrase rules', () => {
  it('accepts a phrase with letters and digits', () => {
    expect(passphraseIssue('Correct-Horse-9')).toBeNull();
    expect(isPassphraseAcceptable('maktab2026tawtiq')).toBe(true);
  });

  it('accepts an Arabic phrase with digits (no letter case exists in Arabic)', () => {
    expect(passphraseIssue('مكتب التوثيق 2026')).toBeNull();
  });

  it('reports each unmet rule', () => {
    expect(passphraseIssue('court')).toBe('setup.tooShort');
    expect(passphraseIssue('alllowercaseletters')).toBe('setup.weak');
    expect(passphraseIssue('password1234')).toBe('setup.common');
    expect(passphraseIssue('aaaaaaaaaaaaaa')).toBe('setup.common');
  });

  it('treats a space as a separator, not a symbol', () => {
    expect(passphraseChecks('mot de passe long').mixed).toBe(false);
  });

  it('accepts a long word phrase without digits', () => {
    expect(passphraseIssue('avoir la foi de mon pere')).toBeNull();
    expect(passphraseIssue('بسم الله الرحمن الرحيم')).toBeNull();
  });

  it('suggests a phrase that passes every rule', () => {
    for (let i = 0; i < 20; i += 1) {
      const phrase = suggestPassphrase();
      expect(phrase.length).toBeGreaterThanOrEqual(12);
      expect(passphraseIssue(phrase)).toBeNull();
    }
  });
});
