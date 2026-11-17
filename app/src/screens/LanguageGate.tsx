import type { JSX } from 'react';
import type { Lang } from '../i18n';
import { direction, t } from '../i18n';

export interface LanguageGateProps {
  onChoose: (lang: Lang) => void;
}

/**
 * One screen, two choices. The choice is stored per user and can be changed later from the top bar
 * without restarting or losing data.
 */
export function LanguageGate({ onChoose }: LanguageGateProps): JSX.Element {
  return (
    <main className="gate">
      <div className="gate-card">
        <span className="brand-mark brand-mark-large" aria-hidden="true">
          ن
        </span>
        {/* Bilingual on purpose: nobody has chosen a language yet. */}
        <h1>{t('ar', 'firstRun.title')}</h1>
        <p>{t('ar', 'firstRun.subtitle')}</p>
        <h1 lang="fr" dir="ltr" className="gate-secondary-title">
          {t('fr', 'firstRun.title')}
        </h1>
        <p lang="fr" dir="ltr">
          {t('fr', 'firstRun.subtitle')}
        </p>
        <div className="gate-choices">
          <button
            type="button"
            className="gate-choice"
            lang="ar"
            dir={direction('ar')}
            onClick={() => onChoose('ar')}
          >
            {t('ar', 'firstRun.arabic')}
          </button>
          <button
            type="button"
            className="gate-choice"
            lang="fr"
            dir={direction('fr')}
            onClick={() => onChoose('fr')}
          >
            {t('fr', 'firstRun.french')}
          </button>
        </div>
      </div>
    </main>
  );
}
