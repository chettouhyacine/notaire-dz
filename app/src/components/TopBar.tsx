import type { JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { LANGS, direction, t } from '../i18n';
import { IconSearch } from './icons';

export interface TopBarProps {
  lang: Lang;
  onLanguageChange: (lang: Lang) => void;
  userLabel: string;
  onSignOut: () => void;
  /** Shown while the shell runs in demo mode, so a demo is never mistaken for the real office. */
  demo?: boolean;
}

export function TopBar({ lang, onLanguageChange, userLabel, onSignOut, demo }: TopBarProps): JSX.Element {
  const label = (key: MessageKey): string => t(lang, key);
  return (
    <header className="topbar">
      <div className="topbar-brand">
        <span className="brand-mark" aria-hidden="true">
          ن
        </span>
        <span className="brand-text">
          <strong>{label('app.name')}</strong>
          <small>{label('app.tagline')}</small>
        </span>
      </div>

      <div className="topbar-search">
        <label className="visually-hidden" htmlFor="global-search">
          {label('topbar.search')}
        </label>
        <span className="search-icon" aria-hidden="true">
          <IconSearch />
        </span>
        <input id="global-search" type="search" placeholder={label('topbar.search')} disabled />
      </div>

      <div className="topbar-actions">
        {demo ? <span className="demo-chip">{label('demo.badge')}</span> : null}
        <div className="lang-switch" role="group" aria-label={label('topbar.language')}>
          {LANGS.map((code) => (
            <button
              key={code}
              type="button"
              className="lang-button"
              aria-pressed={lang === code}
              lang={code}
              dir={direction(code)}
              onClick={() => onLanguageChange(code)}
            >
              {code === 'ar' ? 'العربية' : 'Français'}
            </button>
          ))}
        </div>
        <span className="topbar-user" title={label('topbar.user')}>
          {userLabel}
        </span>
        <button type="button" className="link-button" onClick={onSignOut}>
          {label('topbar.logout')}
        </button>
      </div>
    </header>
  );
}
