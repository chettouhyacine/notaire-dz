import { describe, expect, it } from 'vitest';
import { formatDate, formatMoney, toArabicIndic } from './format';

describe('money formatting', () => {
  it('groups dinars and keeps two decimals', () => {
    expect(formatMoney(12_500_050, 'fr')).toBe('125\u00a0000,50');
    expect(formatMoney(0, 'ar')).toBe('0,00');
    expect(formatMoney(-500, 'fr')).toBe('-5,00');
  });

  it('switches to Arabic-Indic digits on request', () => {
    expect(toArabicIndic('1234')).toBe('١٢٣٤');
    expect(formatMoney(12_500_050, 'ar', 'arabicIndic')).toBe('١٢٥\u00a0٠٠٠,٥٠');
  });
});

describe('date formatting', () => {
  it('renders day/month/year without timezone drift', () => {
    expect(formatDate('2026-03-09', 'fr')).toBe('09/03/2026');
    expect(formatDate('2026-03-09', 'ar', 'arabicIndic')).toBe('٠٩/٠٣/٢٠٢٦');
  });

  it('passes through anything that is not an ISO date', () => {
    expect(formatDate('not-a-date', 'ar')).toBe('not-a-date');
  });
});
