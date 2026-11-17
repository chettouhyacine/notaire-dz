import { useMemo, useState, type FormEvent, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import { initializeOffice, isDesktopShell, type Principal } from '../lib/api';
import {
  passphraseChecks,
  passphraseIssue,
  suggestPassphrase,
  type PassphraseIssue,
} from '../lib/passphrase';

export interface SetupProps {
  lang: Lang;
  onReady: (principal: Principal) => void;
}

type ErrorKey =
  | PassphraseIssue
  | 'setup.mismatch'
  | 'setup.required'
  | 'setup.previewOnly'
  | 'setup.error.exists'
  | 'setup.error.generic';

/**
 * First run, in four fields.
 *
 * The username is fixed (`notaire`) because this build has one office account; asking for it was a
 * field the notary had no reason to fill. The passphrase rules are shown live, the phrase can be
 * revealed, and one button proposes a phrase — so nothing fails silently with "invalid data".
 */
export function Setup({ lang, onReady }: SetupProps): JSX.Element {
  const [officeName, setOfficeName] = useState('');
  const [notaryName, setNotaryName] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [revealed, setRevealed] = useState(false);
  const [suggested, setSuggested] = useState(false);
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Set once the office exists: the recovery code is shown here, once, before the app opens.
  const [created, setCreated] = useState<{ code: string; principal: Principal } | null>(null);
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  const checks = useMemo(() => passphraseChecks(passphrase), [passphrase]);
  const mismatch = confirm.length > 0 && confirm !== passphrase;

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setDetail(null);
    if (!officeName.trim()) {
      setErrorKey('setup.required');
      return;
    }
    const issue = passphraseIssue(passphrase);
    if (issue) {
      setErrorKey(issue);
      return;
    }
    if (passphrase !== confirm) {
      setErrorKey('setup.mismatch');
      return;
    }
    if (!isDesktopShell()) {
      // Browser preview (`?setup`) is for looking at this screen, not for creating an office.
      setErrorKey('setup.previewOnly');
      return;
    }
    setBusy(true);
    setErrorKey(null);
    try {
      const result = await initializeOffice({
        officeName,
        notaryDisplayName: notaryName.trim() || officeName.trim(),
        username: 'admin',
        passphrase,
      });
      setCreated({ code: result.recoveryCode, principal: result.principal });
    } catch (error) {
      const raw = (error as { detail?: string; code?: string } | null) ?? {};
      const message = String(raw.detail ?? '');
      setErrorKey(mapError(message));
      setDetail(message || null);
    } finally {
      setBusy(false);
    }
  }

  function useSuggestion(): void {
    const phrase = suggestPassphrase();
    setPassphrase(phrase);
    setConfirm(phrase);
    setRevealed(true);
    setSuggested(true);
    setErrorKey(null);
  }

  if (created) {
    return (
      <main className="gate">
        <div className="gate-card">
          <h1>{label('setup.recovery.title')}</h1>
          <p>{label('setup.recovery.body')}</p>
          <code className="code-block">{created.code}</code>
          <button
            type="button"
            className="secondary-button"
            onClick={() => {
              void copyText(created.code).then(setCopied);
            }}
          >
            {copied ? label('setup.recovery.copied') : label('setup.recovery.copy')}
          </button>
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={saved}
              onChange={(event) => setSaved(event.target.checked)}
            />
            {label('setup.recovery.saved')}
          </label>
          <button
            type="button"
            className="primary-button"
            disabled={!saved}
            onClick={() => onReady(created.principal)}
          >
            {label('setup.recovery.continue')}
          </button>
        </div>
      </main>
    );
  }

  return (
    <main className="gate">
      <form className="gate-card" onSubmit={handleSubmit} noValidate>
        <span className="brand-mark brand-mark-large" aria-hidden="true">
          ن
        </span>
        <h1>{label('setup.title')}</h1>
        <p>{label('setup.subtitle')}</p>

        <div className="field">
          <label htmlFor="officeName">{label('setup.officeName')}</label>
          <input
            id="officeName"
            value={officeName}
            onChange={(event) => setOfficeName(event.target.value)}
            autoFocus
            required
          />
        </div>

        <div className="field">
          <label htmlFor="notaryName">
            {label('setup.notaryName')} <span className="field-optional">{label('setup.optional')}</span>
          </label>
          <input
            id="notaryName"
            value={notaryName}
            onChange={(event) => setNotaryName(event.target.value)}
          />
        </div>

        <div className="field">
          <div className="field-head">
            <label htmlFor="passphrase">{label('setup.passphrase')}</label>
            <button
              type="button"
              className="link-button"
              onClick={() => setRevealed((value) => !value)}
            >
              {revealed ? label('setup.hide') : label('setup.show')}
            </button>
          </div>
          <input
            id="passphrase"
            type={revealed ? 'text' : 'password'}
            autoComplete="new-password"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
            required
          />
          <ul className="rule-list">
            <Rule ok={checks.length} text={label('setup.rule.length')} />
            <Rule ok={checks.mixed} text={label('setup.rule.mix')} />
            <Rule ok={checks.notRepeated && checks.notListed} text={label('setup.rule.common')} />
          </ul>
          <button type="button" className="link-button" onClick={useSuggestion}>
            {label('setup.suggest')}
          </button>
          {suggested ? <p className="field-help">{label('setup.suggestHint')}</p> : null}
        </div>

        <div className="field">
          <label htmlFor="passphraseConfirm">{label('setup.passphraseConfirm')}</label>
          <input
            id="passphraseConfirm"
            type={revealed ? 'text' : 'password'}
            autoComplete="new-password"
            value={confirm}
            onChange={(event) => setConfirm(event.target.value)}
            required
          />
          {mismatch ? (
            <p className="form-error" role="alert">
              {label('setup.mismatch')}
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
          {busy ? label('setup.busy') : label('setup.submit')}
        </button>
      </form>
    </main>
  );
}

function Rule({ ok, text }: { ok: boolean; text: string }): JSX.Element {
  return (
    <li className={ok ? 'rule-ok' : undefined}>
      <span aria-hidden="true">{ok ? '✓' : '•'}</span> {text}
    </li>
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

/** Core validation messages are human text; map the ones this screen can cause. */
function mapError(message: string): ErrorKey {
  if (message.includes('already initialised')) return 'setup.error.exists';
  if (message.includes('office name')) return 'setup.required';
  if (message.includes('at least')) return 'setup.tooShort';
  if (message.includes('mix')) return 'setup.weak';
  if (message.includes('common')) return 'setup.common';
  if (message.includes('repeat')) return 'setup.common';
  return 'setup.error.generic';
}
