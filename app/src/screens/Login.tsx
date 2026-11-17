import { useState, type FormEvent, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import { signIn, type Principal } from '../lib/api';

export interface LoginProps {
  lang: Lang;
  onSignedIn: (principal: Principal) => void;
  onLanguageChange: (lang: Lang) => void;
  /** Returns to the welcome page; the credential form is never a dead end. */
  onBack?: () => void;
}

type ErrorKey = 'login.error.invalid' | 'login.error.locked';

export function Login({ lang, onSignedIn, onLanguageChange, onBack }: LoginProps): JSX.Element {
  // The office account is created as `admin` / `admin`; both prefilled so signing in is one click.
  const [username, setUsername] = useState('admin');
  const [password, setPassword] = useState('admin');
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [busy, setBusy] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setBusy(true);
    setErrorKey(null);
    const outcome = await signIn(username, password);
    setBusy(false);
    if (outcome.ok && outcome.principal) {
      onSignedIn(outcome.principal);
    } else {
      setErrorKey(outcome.errorKey ?? 'login.error.invalid');
    }
  }

  return (
    <main className="gate">
      <form className="gate-card" onSubmit={handleSubmit} noValidate>
        <span className="brand-mark brand-mark-large" aria-hidden="true">
          ن
        </span>
        <h1>{label('login.title')}</h1>
        <p>{label('login.subtitle')}</p>

        <div className="field">
          <label htmlFor="username">{label('login.username')}</label>
          <input
            id="username"
            name="username"
            autoComplete="username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            required
          />
        </div>

        <div className="field">
          <label htmlFor="password">{label('login.password')}</label>
          <input
            id="password"
            name="password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            required
          />
        </div>

        {errorKey ? (
          <p className="form-error" role="alert">
            {label(errorKey)}
          </p>
        ) : null}

        <button type="submit" className="primary-button" disabled={busy}>
          {label('login.submit')}
        </button>

        {onBack ? (
          <button type="button" className="link-button" onClick={onBack}>
            {label('common.back')}
          </button>
        ) : null}

        <p className="hint">{label('login.demoHint')}</p>

        <div className="gate-language">
          <button type="button" className="link-button" onClick={() => onLanguageChange(lang === 'ar' ? 'fr' : 'ar')}>
            {lang === 'ar' ? 'Français' : 'العربية'}
          </button>
        </div>
      </form>
    </main>
  );
}
