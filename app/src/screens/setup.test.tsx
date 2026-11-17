import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Setup } from './Setup';
import { t } from '../i18n';

afterEach(cleanup);

describe('setup screen', () => {
  it('asks for four fields and no username', () => {
    render(<Setup lang="ar" onReady={() => {}} />);
    expect(screen.getByLabelText(t('ar', 'setup.officeName'))).toBeTruthy();
    expect(screen.getByLabelText(new RegExp(t('ar', 'setup.notaryName')))).toBeTruthy();
    expect(screen.getByLabelText(t('ar', 'setup.passphrase'))).toBeTruthy();
    expect(screen.getByLabelText(t('ar', 'setup.passphraseConfirm'))).toBeTruthy();
    expect(screen.queryByLabelText(t('ar', 'login.username'))).toBeNull();
  });

  it('refuses to create the office before the rules are met, and says which one', () => {
    render(<Setup lang="fr" onReady={() => {}} />);
    const phrase = screen.getByLabelText(t('fr', 'setup.passphrase'));
    const confirm = screen.getByLabelText(t('fr', 'setup.passphraseConfirm'));
    const submit = screen.getByRole('button', { name: t('fr', 'setup.submit') });

    // The office name is the only required free-text field.
    fireEvent.change(screen.getByLabelText(t('fr', 'setup.officeName')), {
      target: { value: 'Étude Test' },
    });
    fireEvent.change(phrase, { target: { value: 'court' } });
    fireEvent.change(confirm, { target: { value: 'court' } });
    fireEvent.click(submit);
    expect(screen.getByRole('alert').textContent).toBe(t('fr', 'setup.tooShort'));

    fireEvent.change(phrase, { target: { value: 'alllowercaseletters' } });
    fireEvent.change(confirm, { target: { value: 'alllowercaseletters' } });
    fireEvent.click(submit);
    expect(screen.getByRole('alert').textContent).toBe(t('fr', 'setup.weak'));
  });

  it('reports a mismatch while typing', () => {
    render(<Setup lang="ar" onReady={() => {}} />);
    fireEvent.change(screen.getByLabelText(t('ar', 'setup.passphrase')), {
      target: { value: 'maktab-2026-tawtiq' },
    });
    fireEvent.change(screen.getByLabelText(t('ar', 'setup.passphraseConfirm')), {
      target: { value: 'maktab-2026' },
    });
    expect(screen.getByRole('alert').textContent).toBe(t('ar', 'setup.mismatch'));
  });

  it('fills both fields from the suggestion button', () => {
    render(<Setup lang="ar" onReady={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: t('ar', 'setup.suggest') }));
    const phrase = screen.getByLabelText<HTMLInputElement>(t('ar', 'setup.passphrase'));
    const confirm = screen.getByLabelText<HTMLInputElement>(t('ar', 'setup.passphraseConfirm'));
    expect(phrase.value).toBe(confirm.value);
    expect(phrase.value.length).toBeGreaterThanOrEqual(12);
    // The suggestion is revealed so the notary can write it down.
    expect(phrase.type).toBe('text');
  });
});
