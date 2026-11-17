import { useMemo, useState, type FormEvent, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import { isDesktopShell, resetPassphrase } from '../lib/api';
import { passphraseChecks, passphraseIssue, type PassphraseIssue } from '../lib/passphrase';

export interface ForgotPassphraseProps {
  lang: Lang;
  /** False for an office created before recovery codes existed. */
  recoveryAvailable: boolean;
  onBack: () => void;
}

type ErrorKey =
  | PassphraseIssue
  | 'forgot.mismatch'
  | 'forgot.error.badCode'
  | 'forgot.error.noRecovery'
  | 'forgot.error.generic'
  | 'forgot.previewOnly';

export function ForgotPassphrase({ lang, recoveryAvailable, onBack }: ForgotPassphraseProps): JSX.Element {
  const [username, setUsername] = useState('notaire');
  const [code, setCode] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [revealed, setRevealed] = useState(false);
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [newCode, setNewCode] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  const checks = useMemo(() => passphraseChecks(passphrase), [passphrase]);
  const mismatch = confirm.length > 0 && confirm !== passphrase;

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setDetail(null);
    if (!code.trim()) {
      setErrorKey('forgot.error.badCode');
      return;
    }
    const issue = passphraseIssue(passphrase);
    if (issue) {
      setErrorKey(issue);
      return;
    }
    if (passphrase !== confirm) {
      setErrorKey('forgot.mismatch');
      return;
    }
    if (!isDesktopShell()) {
      setErrorKey('forgot.previewOnly');
      return;
    }
    setBusy(true);
    setErrorKey(null);
    try {
      const issued = await resetPassphrase(username.trim() || 'notaire', code, passphrase);
      setNewCode(issued);
    } catch (error) {
      const message = String((error as { detail?: string } | null)?.detail ?? '');
      setErrorKey(mapError(message));
      setDetail(message || null);
    } finally {
      setBusy(false);
    }
  }

  if (newCode) {
    return (
      <main className="gate">
        <div className="gate-card">
          <h1>{label('forgot.success.title')}</h1>
          <p>{label('forgot.success.body')}</p>
          <code className="code-block">{newCode}</code>
          <button
            type="button"
            className="secondary-button"
            onClick={() => {
              void copyText(newCode).then((ok) => setCopied(ok));
            }}
          >
            {copied ? label('forgot.copied') : label('forgot.copy')}
          </button>
          <button type="button" className="primary-button" onClick={onBack}>
            {label('forgot.success.continue')}
          </button>
        </div>
      </main>
    );
  }

  return (
    <main className="gate">
      <form className="gate-card" onSubmit={handleSubmit} noValidate>
        <h1>{label('forgot.title')}</h1>
        <p>{label('forgot.subtitle')}</p>

        {!recoveryAvailable ? (
          <p className="form-hint-box" role="note">
            {label('forgot.error.noRecovery')}
          </p>
        ) : null}

        <div className="field">
          <label htmlFor="recoveryUser">{label('login.username')}</label>
          <input
            id="recoveryUser"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            autoComplete="username"
          />
        </div>

        <div className="field">
          <label htmlFor="recoveryCode">{label('forgot.code')}</label>
          <input
            id="recoveryCode"
            className="code-input"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            placeholder="0000-0000-0000-0000-0000-0000"
            autoComplete="off"
            spellCheck={false}
            required
          />
          <p className="field-help">{label('forgot.codeHelp')}</p>
        </div>

        <div className="field">
          <div className="field-head">
            <label htmlFor="newPassphrase">{label('setup.passphrase')}</label>
            <button type="button" className="link-button" onClick={() => setRevealed((v) => !v)}>
              {revealed ? label('setup.hide') : label('setup.show')}
            </button>
          </div>
          <input
            id="newPassphrase"
            type={revealed ? 'text' : 'password'}
            autoComplete="new-password"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
            required
          />
          <ul className="rule-list">
            <li className={checks.length ? 'rule-ok' : undefined}>
              <span aria-hidden="true">{checks.length ? '✓' : '•'}</span> {label('setup.rule.length')}
            </li>
            <li className={checks.mixed ? 'rule-ok' : undefined}>
              <span aria-hidden="true">{checks.mixed ? '✓' : '•'}</span> {label('setup.rule.mix')}
            </li>
            <li className={checks.notRepeated && checks.notListed ? 'rule-ok' : undefined}>
              <span aria-hidden="true">{checks.notRepeated && checks.notListed ? '✓' : '•'}</span>{' '}
              {label('setup.rule.common')}
            </li>
          </ul>
        </div>

        <div className="field">
          <label htmlFor="newPassphraseConfirm">{label('setup.passphraseConfirm')}</label>
          <input
            id="newPassphraseConfirm"
            type={revealed ? 'text' : 'password'}
            autoComplete="new-password"
            value={confirm}
            onChange={(event) => setConfirm(event.target.value)}
            required
          />
          {mismatch ? (
            <p className="form-error" role="alert">
              {label('forgot.mismatch')}
            </p>
          ) : null}
        </div>

        {errorKey ? (
          <p className="form-error" role="alert">
            {label(errorKey)}
          </p>
        ) : null}
        {detail ? <p className="field-help mono">{detail}</p> : null}

        <button type="submit" className="primary-button" disabled={busy}>
          {busy ? label('forgot.busy') : label('forgot.submit')}
        </button>
        <button type="button" className="link-button" onClick={onBack}>
          {label('common.back')}
        </button>
        <p className="hint">{label('forgot.noCodeHint')}</p>
      </form>
    </main>
  );
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

function mapError(message: string): ErrorKey {
  if (!message) return 'forgot.error.generic';
  if (message.includes('no recovery code')) return 'forgot.error.noRecovery';
  if (message.includes('recovery code is incorrect')) return 'forgot.error.badCode';
  if (message.includes('at least')) return 'setup.tooShort';
  if (message.includes('mix')) return 'setup.weak';
  if (message.includes('common') || message.includes('repeat')) return 'setup.common';
  return 'forgot.error.generic';
}
