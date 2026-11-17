import { useEffect, useState, type FormEvent, type JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';
import {
  createPerson,
  isDesktopShell,
  listParties,
  searchParties,
  type PartyDto,
} from '../lib/api';

export interface ClientsProps {
  lang: Lang;
}

type ErrorKey = 'clients.missingName' | 'settings.desktopOnly' | 'error.storage';

function PersonForm({
  lang,
  onSaved,
  onCancel,
}: {
  lang: Lang;
  onSaved: () => void;
  onCancel: () => void;
}): JSX.Element {
  const [surnameLatin, setSurnameLatin] = useState('');
  const [givenLatin, setGivenLatin] = useState('');
  const [surnameAr, setSurnameAr] = useState('');
  const [givenAr, setGivenAr] = useState('');
  const [birthDate, setBirthDate] = useState('');
  const [nin, setNin] = useState('');
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [busy, setBusy] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!surnameLatin.trim() || !givenLatin.trim()) {
      setErrorKey('clients.missingName');
      return;
    }
    if (!isDesktopShell()) {
      setErrorKey('settings.desktopOnly');
      return;
    }
    setBusy(true);
    setErrorKey(null);
    try {
      await createPerson({ surnameLatin, givenLatin, surnameAr, givenAr, birthDate, nin });
      onSaved();
    } catch {
      setErrorKey('error.storage');
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="panel" onSubmit={handleSubmit} noValidate>
      <legend>{label('clients.newTitle')}</legend>

      <div className="field">
        <label htmlFor="surnameLatin">{label('clients.fieldSurnameLatin')}</label>
        <input id="surnameLatin" value={surnameLatin} onChange={(e) => setSurnameLatin(e.target.value)} />
      </div>

      <div className="field">
        <label htmlFor="givenLatin">{label('clients.fieldGivenLatin')}</label>
        <input id="givenLatin" value={givenLatin} onChange={(e) => setGivenLatin(e.target.value)} />
      </div>

      <div className="field">
        <label htmlFor="surnameAr">{label('clients.fieldSurnameAr')}</label>
        <input id="surnameAr" lang="ar" dir="rtl" value={surnameAr} onChange={(e) => setSurnameAr(e.target.value)} />
      </div>

      <div className="field">
        <label htmlFor="givenAr">{label('clients.fieldGivenAr')}</label>
        <input id="givenAr" lang="ar" dir="rtl" value={givenAr} onChange={(e) => setGivenAr(e.target.value)} />
      </div>

      <div className="field">
        <label htmlFor="birthDate">{label('clients.fieldBirthDate')}</label>
        <input id="birthDate" type="date" value={birthDate} onChange={(e) => setBirthDate(e.target.value)} />
      </div>

      <div className="field">
        <label htmlFor="nin">{label('clients.fieldNin')}</label>
        <input id="nin" inputMode="numeric" value={nin} onChange={(e) => setNin(e.target.value)} />
      </div>

      {errorKey ? (
        <p className="form-error" role="alert">
          {label(errorKey)}
        </p>
      ) : null}

      <div className="choice-row">
        <button type="submit" className="primary-button" disabled={busy}>
          {label('clients.save')}
        </button>
        <button type="button" className="chip" onClick={onCancel}>
          {label('common.cancel')}
        </button>
      </div>
    </form>
  );
}

export function Clients({ lang }: ClientsProps): JSX.Element {
  const [parties, setParties] = useState<PartyDto[]>([]);
  const [query, setQuery] = useState('');
  const [showForm, setShowForm] = useState(false);
  const [saved, setSaved] = useState(false);
  const label = (key: MessageKey): string => t(lang, key);

  async function refresh(): Promise<void> {
    try {
      setParties(await searchParties(query));
    } catch {
      setParties([]);
    }
  }

  useEffect(() => {
    let cancelled = false;
    listParties()
      .then((rows) => {
        if (!cancelled) setParties(rows);
      })
      .catch(() => {
        if (!cancelled) setParties([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSearch(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    await refresh();
  }

  return (
    <section className="screen">
      <header className="screen-header">
        <div>
          <h1>{label('clients.title')}</h1>
          <p className="subtitle">{label('clients.subtitle')}</p>
        </div>
        <button type="button" className="primary-button" onClick={() => setShowForm((v) => !v)}>
          + {label('clients.newTitle')}
        </button>
      </header>

      {saved ? (
        <p className="field-help" role="status">
          {label('clients.saved')}
        </p>
      ) : null}

      {showForm ? (
        <PersonForm
          lang={lang}
          onCancel={() => setShowForm(false)}
          onSaved={() => {
            setShowForm(false);
            setSaved(true);
            void refresh();
          }}
        />
      ) : null}

      <form className="search-row" onSubmit={handleSearch}>
        <div className="field grow">
          <label htmlFor="clientSearch">{label('clients.search')}</label>
          <input
            id="clientSearch"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={label('clients.searchHint')}
          />
        </div>
        <button type="submit" className="chip">
          {label('clients.searchAction')}
        </button>
      </form>

      <h2 className="section-title">{label('clients.results')}</h2>
      {parties.length === 0 ? (
        <p className="hint">{label('clients.searchEmpty')}</p>
      ) : (
        <ul className="result-list">
          {parties.map((party) => (
            <li key={party.id} className="result-row">
              <span className="result-primary" dir="auto">
                {party.displayLatin || party.displayAr}
              </span>
              <span className="result-secondary" dir="auto">
                {party.displayAr}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
