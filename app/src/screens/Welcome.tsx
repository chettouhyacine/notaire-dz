import type { JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';

export interface WelcomeProps {
  lang: Lang;
  onSignIn: () => void;
  onForgot: () => void;
  onLanguageChange: (lang: Lang) => void;
}

/**
 * The first screen of a working day: who this is, and the two doors — sign in, or recover access.
 * Kept separate from the credential form so the notary sees the way back *before* getting stuck.
 */
export function Welcome({ lang, onSignIn, onForgot, onLanguageChange }: WelcomeProps): JSX.Element {
  const label = (key: MessageKey): string => t(lang, key);

  return (
    <main className="gate">
      <div className="gate-card">
        <span className="brand-mark brand-mark-large" aria-hidden="true">
          ن
        </span>
        <h1>{label('app.name')}</h1>
        <p>{label('welcome.subtitle')}</p>

        <button type="button" className="primary-button" onClick={onSignIn}>
          {label('welcome.signIn')}
        </button>

        <button type="button" className="link-button" onClick={onForgot}>
          {label('welcome.forgot')}
        </button>

        <p className="hint">{label('welcome.dataNote')}</p>

        <div className="gate-language">
          <button
            type="button"
            className="link-button"
            onClick={() => onLanguageChange(lang === 'ar' ? 'fr' : 'ar')}
          >
            {lang === 'ar' ? 'Français' : 'العربية'}
          </button>
        </div>
      </div>
    </main>
  );
}
