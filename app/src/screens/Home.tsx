import type { JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import type { DigitStyle } from '../lib/format';
import { formatMoney, toArabicIndic } from '../lib/format';

export interface HomeCounts {
  appointments: number;
  waiting: number;
  unpaidCentimes: number;
  expiring: number;
}

export interface HomeProps {
  lang: Lang;
  digits: DigitStyle;
  counts: HomeCounts;
  onNewCase: () => void;
  onAccount: () => void;
}

export function Home({ lang, digits, counts, onNewCase, onAccount }: HomeProps): JSX.Element {
  const label = (key: MessageKey): string => t(lang, key);
  const num = (value: number): string =>
    digits === 'arabicIndic' ? toArabicIndic(String(value)) : String(value);

  const cards = [
    {
      key: 'home.cards.appointments' as MessageKey,
      hint: 'home.cards.appointmentsHint' as MessageKey,
      value: num(counts.appointments),
    },
    {
      key: 'home.cards.waiting' as MessageKey,
      hint: 'home.cards.waitingHint' as MessageKey,
      value: num(counts.waiting),
    },
    {
      key: 'home.cards.unpaid' as MessageKey,
      hint: 'home.cards.unpaidHint' as MessageKey,
      value: formatMoney(counts.unpaidCentimes, lang, digits),
    },
    {
      key: 'home.cards.expiring' as MessageKey,
      hint: 'home.cards.expiringHint' as MessageKey,
      value: num(counts.expiring),
    },
  ];

  return (
    <section className="screen">
      <header className="screen-header">
        <div>
          <h1>{label('home.title')}</h1>
          <p className="subtitle">{label('home.subtitle')}</p>
        </div>
      </header>

      <div className="cards">
        {cards.map((card) => (
          <article className="card" key={card.key}>
            <h2>{label(card.key)}</h2>
            <p className="card-value">{card.value}</p>
            <p className="card-hint">{label(card.hint)}</p>
          </article>
        ))}
      </div>

      <div className="primary-action">
        <button type="button" className="primary-button large" onClick={onNewCase}>
          + {label('home.newCase')}
        </button>
        <button type="button" className="secondary-button" onClick={onAccount}>
          {label('home.account')}
        </button>
      </div>

      <p className="phase-note">{label('home.phaseNote')}</p>
    </section>
  );
}
