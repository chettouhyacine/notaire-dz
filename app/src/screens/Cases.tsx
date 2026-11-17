import { useEffect, useState, type FormEvent, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import { createCase, isDesktopShell, listCases, type CaseDto } from '../lib/api';

export interface CasesProps {
  lang: Lang;
  userId: string;
}

const ACT_TYPES = [
  'sale',
  'donation',
  'fridha',
  'poa',
  'lease',
  'will',
  'marriage',
  'company',
] as const;

const STAGES = [
  'intake',
  'documents',
  'verification',
  'drafting',
  'review',
  'reading',
  'signature',
  'registration',
  'publication',
  'delivery',
  'archived',
] as const;

function stageLabel(lang: Lang, stage: string): string {
  const known = (STAGES as readonly string[]).includes(stage);
  return known ? t(lang, `stage.${stage}` as MessageKey) : stage;
}

function actTypeLabel(lang: Lang, actType: string): string {
  const known = (ACT_TYPES as readonly string[]).includes(actType);
  return known ? t(lang, `acttype.${actType}` as MessageKey) : actType;
}

export function Cases({ lang, userId }: CasesProps): JSX.Element {
  const [cases, setCases] = useState<CaseDto[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [actType, setActType] = useState<string>('sale');
  const [dueInDays, setDueInDays] = useState('');
  const [confidential, setConfidential] = useState(false);
  const [errorKey, setErrorKey] = useState<'cases.needsActType' | 'settings.desktopOnly' | 'error.storage' | null>(
    null,
  );
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  async function refresh(): Promise<void> {
    try {
      setCases(await listCases());
    } catch {
      setCases([]);
    }
  }

  useEffect(() => {
    let cancelled = false;
    listCases()
      .then((rows) => {
        if (!cancelled) setCases(rows);
      })
      .catch(() => {
        if (!cancelled) setCases([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!actType) {
      setErrorKey('cases.needsActType');
      return;
    }
    if (!isDesktopShell()) {
      setErrorKey('settings.desktopOnly');
      return;
    }
    setBusy(true);
    setErrorKey(null);
    try {
      await createCase({
        actType,
        ownerUserId: userId,
        dueInDays: dueInDays.trim() ? Number(dueInDays) : null,
        confidential,
      });
      setSaved(true);
      setShowForm(false);
      await refresh();
    } catch {
      setErrorKey('error.storage');
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="screen">
      <header className="screen-header">
        <div>
          <h1>{label('cases.title')}</h1>
          <p className="subtitle">{label('cases.subtitle')}</p>
        </div>
        <button type="button" className="primary-button" onClick={() => setShowForm((v) => !v)}>
          + {label('cases.newCase')}
        </button>
      </header>

      {saved ? (
        <p className="field-help" role="status">
          {label('cases.saved')}
        </p>
      ) : null}

      {showForm ? (
        <form className="panel" onSubmit={handleSubmit} noValidate>
          <fieldset className="plain-fieldset">
            <legend>{label('cases.newCase')}</legend>

            <div className="field">
              <label htmlFor="actType">{label('cases.actType')}</label>
              <select id="actType" value={actType} onChange={(e) => setActType(e.target.value)}>
                {ACT_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {actTypeLabel(lang, type)}
                  </option>
                ))}
              </select>
            </div>

            <div className="field">
              <label htmlFor="dueInDays">{label('cases.dueInDays')}</label>
              <input
                id="dueInDays"
                inputMode="numeric"
                value={dueInDays}
                onChange={(e) => setDueInDays(e.target.value)}
              />
            </div>

            <label className="checkbox-row" htmlFor="confidential">
              <input
                id="confidential"
                type="checkbox"
                checked={confidential}
                onChange={(e) => setConfidential(e.target.checked)}
              />
              <span>{label('cases.confidential')}</span>
            </label>

            {errorKey ? (
              <p className="form-error" role="alert">
                {label(errorKey)}
              </p>
            ) : null}

            <button type="submit" className="primary-button" disabled={busy}>
              {label('cases.save')}
            </button>
          </fieldset>
        </form>
      ) : null}

      <h2 className="section-title">{label('cases.title')}</h2>
      {cases.length === 0 ? (
        <p className="hint">{label('cases.none')}</p>
      ) : (
        <ul className="result-list">
          {cases.map((row) => (
            <li key={row.id} className="result-row">
              <span className="result-primary">{row.reference}</span>
              <span className="result-secondary">{actTypeLabel(lang, row.actType)}</span>
              <span className="badge">{stageLabel(lang, row.stage)}</span>
              {row.confidential ? <span className="badge">{label('cases.confidential')}</span> : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
