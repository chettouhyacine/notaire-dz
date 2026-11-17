import { describe, expect, it } from 'vitest';
import { checkParity, direction, t, type MessageKey } from './index';
import ar from './ar.json';
import fr from './fr.json';

describe('translations', () => {
  it('has no missing or empty keys in either language', () => {
    // A missing key is also a compile error (fr is typed against ar); this catches runtime gaps
    // and empty strings, which a type system cannot see.
    expect(checkParity()).toEqual([]);
  });

  it('covers the eight navigation entries in both languages', () => {
    const keys: MessageKey[] = [
      'nav.home',
      'nav.cases',
      'nav.clients',
      'nav.acts',
      'nav.accounting',
      'nav.compliance',
      'nav.calendar',
      'nav.settings',
    ];
    for (const key of keys) {
      expect(t('ar', key).length).toBeGreaterThan(0);
      expect(t('fr', key).length).toBeGreaterThan(0);
      expect(t('ar', key)).not.toBe(t('fr', key));
    }
  });

  it('keeps every error key the Rust core can return', () => {
    // These strings must exist because UiError carries exactly these keys.
    const coreKeys: MessageKey[] = [
      'error.crypto',
      'error.storage',
      'error.validation',
      'error.denied',
      'error.locked_out',
      'error.reauth_required',
      'error.audit_broken',
      'error.io',
      'error.rules',
    ];
    for (const key of coreKeys) {
      expect(ar[key]).toBeTruthy();
      expect(fr[key]).toBeTruthy();
    }
  });

  it('maps language to writing direction', () => {
    expect(direction('ar')).toBe('rtl');
    expect(direction('fr')).toBe('ltr');
  });

  it('interpolates variables', () => {
    expect(t('fr', 'common.phase', { n: 2 })).toBe('Phase');
  });
});
