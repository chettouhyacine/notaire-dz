import { useState, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';

export interface OnboardingProps {
  lang: Lang;
  onDone: () => void;
}

/** Five short pages: what the program delivers, how to work with it, and how it is protected. */
const PAGES: readonly { title: MessageKey; body: MessageKey; points: MessageKey[] }[] = [
  {
    title: 'onboarding.p1.title',
    body: 'onboarding.p1.body',
    points: ['onboarding.p1.a', 'onboarding.p1.b', 'onboarding.p1.c'],
  },
  {
    title: 'onboarding.p2.title',
    body: 'onboarding.p2.body',
    points: ['onboarding.p2.a', 'onboarding.p2.b', 'onboarding.p2.c'],
  },
  {
    title: 'onboarding.p3.title',
    body: 'onboarding.p3.body',
    points: ['onboarding.p3.a', 'onboarding.p3.b'],
  },
  {
    title: 'onboarding.p4.title',
    body: 'onboarding.p4.body',
    points: ['onboarding.p4.a', 'onboarding.p4.b', 'onboarding.p4.c'],
  },
  {
    title: 'onboarding.p5.title',
    body: 'onboarding.p5.body',
    points: ['onboarding.p5.a', 'onboarding.p5.b'],
  },
];

/**
 * Shown once per installation, after the first sign-in. Skippable at any page; the flag is written
 * when it closes, so it never appears again.
 */
export function Onboarding({ lang, onDone }: OnboardingProps): JSX.Element {
  const [index, setIndex] = useState(0);
  const label = (key: MessageKey, vars?: Record<string, string | number>): string =>
    t(lang, key, vars);
  const page = PAGES[index]!;
  const last = index === PAGES.length - 1;

  return (
    <main className="gate">
      <section className="gate-card onboarding-card" aria-label={label('onboarding.aria')}>
        <div className="onboarding-top">
          <span className="onboarding-step">
            {label('onboarding.step', { current: index + 1, total: PAGES.length })}
          </span>
          <button type="button" className="link-button" onClick={onDone}>
            {label('onboarding.skip')}
          </button>
        </div>

        <h1>{label(page.title)}</h1>
        <p>{label(page.body)}</p>
        <ul className="onboarding-list">
          {page.points.map((point) => (
            <li key={point}>{label(point)}</li>
          ))}
        </ul>

        <div className="onboarding-dots" aria-hidden="true">
          {PAGES.map((_, dot) => (
            <span key={dot} className={dot === index ? 'dot dot-active' : 'dot'} />
          ))}
        </div>

        <div className="onboarding-actions">
          {index > 0 ? (
            <button
              type="button"
              className="secondary-button"
              onClick={() => setIndex((value) => Math.max(0, value - 1))}
            >
              {label('onboarding.back')}
            </button>
          ) : null}
          <button
            type="button"
            className="primary-button"
            onClick={() => {
              if (last) onDone();
              else setIndex((value) => value + 1);
            }}
          >
            {last ? label('onboarding.finish') : label('onboarding.next')}
          </button>
        </div>
      </section>
    </main>
  );
}
