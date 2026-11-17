import { useEffect, useState, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import { auditHead, createBackup, isDesktopShell, type AuditReport } from '../lib/api';

/**
 * Shows the audit head hash the notary is expected to countersign daily, and offers an encrypted
 * backup. Both are core commands: the UI only displays what the core reports.
 */
export function SecurityPanel({ lang }: { lang: Lang }): JSX.Element {
  const [report, setReport] = useState<AuditReport | null>(null);
  const [backupState, setBackupState] = useState<'idle' | 'done' | 'failed'>('idle');
  const [passphrase, setPassphrase] = useState('');
  const desktop = isDesktopShell();
  const label = (key: MessageKey): string => t(lang, key);

  useEffect(() => {
    let cancelled = false;
    if (!desktop) return () => undefined;
    auditHead()
      .then((value) => {
        if (!cancelled) setReport(value);
      })
      .catch(() => {
        if (!cancelled) setReport(null);
      });
    return () => {
      cancelled = true;
    };
  }, [desktop]);

  async function handleBackup(): Promise<void> {
    try {
      const ok = await createBackup(passphrase);
      setBackupState(ok ? 'done' : 'failed');
      setPassphrase('');
    } catch {
      setBackupState('failed');
    }
  }

  return (
    <fieldset className="panel">
      <legend>{label('settings.security')}</legend>

      {!desktop ? (
        <p className="field-help">{label('settings.desktopOnly')}</p>
      ) : (
        <>
          <div className="field">
            <span className="field-label">{label('settings.auditHead')}</span>
            <code className="hash">{report?.headHex ?? '—'}</code>
            <p className="field-help">
              {label('settings.auditEvents')}: {report?.events ?? 0} ·{' '}
              {report?.chainOk ? label('settings.auditOk') : label('settings.auditBroken')}
            </p>
          </div>

          <div className="field">
            <label htmlFor="backup-passphrase">{label('settings.createBackup')}</label>
            <input
              id="backup-passphrase"
              type="password"
              autoComplete="current-password"
              value={passphrase}
              onChange={(event) => setPassphrase(event.target.value)}
            />
            <button type="button" className="chip" onClick={() => void handleBackup()}>
              {label('settings.createBackup')}
            </button>
            {backupState === 'done' ? (
              <p className="field-help" role="status">
                {label('settings.backupDone')}
              </p>
            ) : null}
            {backupState === 'failed' ? (
              <p className="form-error" role="alert">
                {label('error.storage')}
              </p>
            ) : null}
          </div>
        </>
      )}
    </fieldset>
  );
}
