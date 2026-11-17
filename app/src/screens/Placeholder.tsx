import type { JSX } from 'react';
import type { Lang, MessageKey } from '../i18n';
import { t } from '../i18n';

export interface PlaceholderProps {
  lang: Lang;
  titleKey: MessageKey;
  subtitleKey: MessageKey;
}

/**
 * Friendly empty screen for a module that is not in this phase. It says so plainly instead of
 * pretending a button would work: the action is disabled and the reason is visible.
 */
export function Placeholder({ lang, titleKey, subtitleKey }: PlaceholderProps): JSX.Element {
  const label = (key: MessageKey): string => t(lang, key);
  return (
    <section className="screen">
      <header className="screen-header">
        <div>
          <h1>{label(titleKey)}</h1>
          <p className="subtitle">{label(subtitleKey)}</p>
        </div>
      </header>

      <div className="empty">
        <p className="empty-title">{label('empty.title')}</p>
        <p className="empty-body">{label('empty.body')}</p>
        <button type="button" className="primary-button" disabled aria-describedby="phase-note">
          {label('empty.action')}
        </button>
        <p id="phase-note" className="hint">
          {label('status.comingPhase2')}
        </p>
      </div>
    </section>
  );
}
