import type { JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { direction, t } from '../i18n';
import type { DigitStyle } from '../lib/format';
import { SecurityPanel } from '../components/SecurityPanel';
import { AccountPanel } from '../components/AccountPanel';
import type { Principal } from '../lib/api';

export interface SettingsProps {
  lang: Lang;
  digits: DigitStyle;
  actLang: Lang;
  username: string;
  onLanguageChange: (lang: Lang) => void;
  onDigitsChange: (digits: DigitStyle) => void;
  onActLangChange: (lang: Lang) => void;
  onAccountRenamed: (principal: Principal) => void;
}

export function Settings({
  lang,
  digits,
  actLang,
  username,
  onLanguageChange,
  onDigitsChange,
  onActLangChange,
  onAccountRenamed,
}: SettingsProps): JSX.Element {
  const label = (key: MessageKey): string => t(lang, key);

  return (
    <section className="screen">
      <header className="screen-header">
        <div>
          <h1>{label('settings.title')}</h1>
          <p className="subtitle">{label('settings.subtitle')}</p>
        </div>
      </header>

      <fieldset className="panel">
        <legend>{label('settings.language')}</legend>

        <div className="field">
          <span className="field-label">{label('settings.interfaceLanguage')}</span>
          <div className="choice-row">
            {(['ar', 'fr'] as const).map((code) => (
              <button
                key={code}
                type="button"
                className="chip"
                aria-pressed={lang === code}
                lang={code}
                dir={direction(code)}
                onClick={() => onLanguageChange(code)}
              >
                {code === 'ar' ? 'العربية' : 'Français'}
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <span className="field-label">{label('settings.actLanguage')}</span>
          <div className="choice-row">
            {(['ar', 'fr'] as const).map((code) => (
              <button
                key={code}
                type="button"
                className="chip"
                aria-pressed={actLang === code}
                onClick={() => onActLangChange(code)}
              >
                {code === 'ar' ? 'العربية' : 'Français'}
              </button>
            ))}
          </div>
          <p className="field-help">{label('settings.note')}</p>
        </div>

        <div className="field">
          <span className="field-label">{label('settings.digits')}</span>
          <div className="choice-row">
            {(['western', 'arabicIndic'] as const).map((style) => (
              <button
                key={style}
                type="button"
                className="chip"
                aria-pressed={digits === style}
                onClick={() => onDigitsChange(style)}
              >
                {style === 'western'
                  ? label('settings.digits.western')
                  : label('settings.digits.arabic')}
              </button>
            ))}
          </div>
        </div>
      </fieldset>

      <AccountPanel lang={lang} username={username} onRenamed={onAccountRenamed} />

      <SecurityPanel lang={lang} />
    </section>
  );
}
