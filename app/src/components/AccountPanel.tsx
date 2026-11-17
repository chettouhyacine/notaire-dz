import { useState, type FormEvent, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import { changePassword, isDesktopShell, renameAccount, type Principal } from '../lib/api';
import { passphraseIssue } from '../lib/passphrase';

export interface AccountPanelProps {
  lang: Lang;
  username: string;
  onRenamed: (principal: Principal) => void;
}

type Message = { kind: 'ok' | 'error'; text: string } | null;

/**
 * Account details, reached from Home.
 *
 * The password here *is* the office passphrase: changing it re-wraps the master key, so the office
 * keeps opening and every record stays readable. The current password is required because an
 * unattended unlocked window must not be enough to lock the owner out.
 */
export function AccountPanel({ lang, username, onRenamed }: AccountPanelProps): JSX.Element {
  const [newUsername, setNewUsername] = useState(username);
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [message, setMessage] = useState<Message>(null);
  const [busy, setBusy] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  async function handleRename(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!isDesktopShell()) {
      setMessage({ kind: 'error', text: label('settings.desktopOnly') });
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const principal = await renameAccount(newUsername.trim());
      onRenamed(principal);
      setMessage({ kind: 'ok', text: label('account.renamed') });
    } catch (error) {
      setMessage({ kind: 'error', text: describe(error, 'account.error.username') });
    } finally {
      setBusy(false);
    }
  }

  async function handlePassword(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const issue = passphraseIssue(next);
    if (issue) {
      setMessage({ kind: 'error', text: label(issue) });
      return;
    }
    if (next !== confirm) {
      setMessage({ kind: 'error', text: label('setup.mismatch') });
      return;
    }
    if (!isDesktopShell()) {
      setMessage({ kind: 'error', text: label('settings.desktopOnly') });
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      await changePassword(current, next);
      setCurrent('');
      setNext('');
      setConfirm('');
      setMessage({ kind: 'ok', text: label('account.passwordChanged') });
    } catch (error) {
      const detail = String((error as { detail?: string } | null)?.detail ?? '');
      setMessage({
        kind: 'error',
        text: detail.includes('current password')
          ? label('account.error.current')
          : describe(error, 'account.error.generic'),
      });
    } finally {
      setBusy(false);
    }
  }

  function describe(error: unknown, fallback: MessageKey): string {
    const detail = String((error as { detail?: string } | null)?.detail ?? '');
    if (detail.includes('already used')) return label('account.error.taken');
    if (detail.includes('at least 3')) return label('account.error.username');
    return fallback === 'account.error.username' ? label(fallback) : label(fallback);
  }

  return (
    <fieldset className="panel">
      <legend>{label('account.title')}</legend>
      <p className="field-help">{label('account.subtitle')}</p>

      <form onSubmit={handleRename} noValidate>
        <div className="field">
          <label htmlFor="accountUsername">{label('account.username')}</label>
          <input
            id="accountUsername"
            value={newUsername}
            onChange={(event) => setNewUsername(event.target.value)}
            autoComplete="username"
          />
        </div>
        <button type="submit" className="secondary-button" disabled={busy}>
          {label('account.saveUsername')}
        </button>
      </form>

      <form onSubmit={handlePassword} noValidate>
        <div className="field">
          <label htmlFor="accountCurrent">{label('account.current')}</label>
          <input
            id="accountCurrent"
            type="password"
            autoComplete="current-password"
            value={current}
            onChange={(event) => setCurrent(event.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor="accountNext">{label('account.next')}</label>
          <input
            id="accountNext"
            type="password"
            autoComplete="new-password"
            value={next}
            onChange={(event) => setNext(event.target.value)}
          />
          <p className="field-help">{label('account.hint')}</p>
        </div>
        <div className="field">
          <label htmlFor="accountConfirm">{label('setup.passphraseConfirm')}</label>
          <input
            id="accountConfirm"
            type="password"
            autoComplete="new-password"
            value={confirm}
            onChange={(event) => setConfirm(event.target.value)}
          />
        </div>
        <button type="submit" className="secondary-button" disabled={busy}>
          {label('account.savePassword')}
        </button>
      </form>

      {message ? (
        <p className={message.kind === 'ok' ? 'form-ok' : 'form-error'} role={message.kind === 'ok' ? 'status' : 'alert'}>
          {message.text}
        </p>
      ) : null}
    </fieldset>
  );
}
