import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Clients } from './Clients';
import { Cases } from './Cases';
import { t } from '../i18n';

beforeEach(() => localStorage.clear());
afterEach(cleanup);

describe('clients screen', () => {
  it('renders the empty state and opens the client form', async () => {
    render(<Clients lang="fr" />);
    expect(screen.getByRole('heading', { level: 1, name: t('fr', 'clients.title') })).toBeTruthy();
    expect(await screen.findByText(t('fr', 'clients.searchEmpty'))).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: new RegExp(t('fr', 'clients.newTitle')) }));
    expect(screen.getByLabelText(t('fr', 'clients.fieldSurnameLatin'))).toBeTruthy();
    expect(screen.getByLabelText(t('fr', 'clients.fieldGivenLatin'))).toBeTruthy();
    expect(screen.getByLabelText(t('fr', 'clients.fieldNin'))).toBeTruthy();
    // Arabic inputs are marked as RTL so mixed names render correctly.
    expect(screen.getByLabelText(t('fr', 'clients.fieldSurnameAr')).getAttribute('dir')).toBe('rtl');
  });

  it('refuses to save without a Latin surname and given name', async () => {
    render(<Clients lang="ar" />);
    fireEvent.click(screen.getByRole('button', { name: new RegExp(t('ar', 'clients.newTitle')) }));
    fireEvent.click(screen.getByRole('button', { name: t('ar', 'clients.save') }));

    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toBe(t('ar', 'clients.missingName')),
    );
  });

  it('says plainly that saving needs the desktop app when running in a browser', async () => {
    render(<Clients lang="fr" />);
    fireEvent.click(screen.getByRole('button', { name: new RegExp(t('fr', 'clients.newTitle')) }));
    fireEvent.change(screen.getByLabelText(t('fr', 'clients.fieldSurnameLatin')), {
      target: { value: 'BENZAKOUR' },
    });
    fireEvent.change(screen.getByLabelText(t('fr', 'clients.fieldGivenLatin')), {
      target: { value: 'Mohamed' },
    });
    fireEvent.click(screen.getByRole('button', { name: t('fr', 'clients.save') }));

    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toBe(t('fr', 'settings.desktopOnly')),
    );
  });
});

describe('cases screen', () => {
  it('lists nothing yet and offers the eight act types', async () => {
    render(<Cases lang="ar" userId="u1" />);
    expect(await screen.findByText(t('ar', 'cases.none'))).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: new RegExp(t('ar', 'cases.newCase')) }));
    const select = screen.getByLabelText(t('ar', 'cases.actType')) as HTMLSelectElement;
    expect(select.options.length).toBe(8);
    expect(select.options[0].textContent).toBe(t('ar', 'acttype.sale'));
    expect(screen.getByLabelText(t('ar', 'cases.confidential'))).toBeTruthy();
  });

  it('requires the desktop app to create a case from the browser preview', async () => {
    render(<Cases lang="fr" userId="u1" />);
    fireEvent.click(screen.getByRole('button', { name: new RegExp(t('fr', 'cases.newCase')) }));
    fireEvent.click(screen.getByRole('button', { name: t('fr', 'cases.save') }));
    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toBe(t('fr', 'settings.desktopOnly')),
    );
  });
});
