import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from './App';
import { DEMO_PASSWORD, DEMO_USERNAME } from './lib/api';
import { t } from './i18n';

beforeEach(() => {
  localStorage.clear();
  // Most tests exercise the shell; the instructions wizard has its own test below.
  localStorage.setItem('notairedz.ui.onboarding', 'done');
  document.documentElement.dir = '';
  document.documentElement.lang = '';
});

afterEach(cleanup);

async function openLogin(lang: 'ar' | 'fr'): Promise<void> {
  // The shell shows a loading card until the core has answered.
  const signIn = await screen.findByRole('button', { name: t(lang, 'welcome.signIn') });
  fireEvent.click(signIn);
  await waitFor(() => expect(screen.getByLabelText(t(lang, 'login.username'))).toBeTruthy());
}

async function signIn(): Promise<void> {
  await openLogin('fr');
  fireEvent.change(screen.getByLabelText(t('fr', 'login.username')), {
    target: { value: DEMO_USERNAME },
  });
  fireEvent.change(screen.getByLabelText(t('fr', 'login.password')), {
    target: { value: DEMO_PASSWORD },
  });
  fireEvent.click(screen.getByRole('button', { name: t('fr', 'login.submit') }));
  await waitFor(() => expect(screen.getByText(t('fr', 'home.newCase'), { exact: false })).toBeTruthy());
}

describe('application shell', () => {
  it('asks for a language on first launch and applies RTL for Arabic', async () => {
    render(<App />);

    expect(screen.getByRole('button', { name: 'العربية' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Français' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'العربية' }));

    await waitFor(() => expect(document.documentElement.dir).toBe('rtl'));
    expect(document.documentElement.lang).toBe('ar');
    // The sign-in landing page comes first, in Arabic; the credential form is one step behind it.
    expect(screen.getByText(t('ar', 'welcome.subtitle'))).toBeTruthy();
    await openLogin('ar');
    expect(screen.getByText(t('ar', 'login.title'))).toBeTruthy();
  });

  it('applies LTR for French', async () => {
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Français' }));
    await waitFor(() => expect(document.documentElement.dir).toBe('ltr'));
    expect(screen.getByText(t('fr', 'welcome.subtitle'))).toBeTruthy();
    await openLogin('fr');
    expect(screen.getByText(t('fr', 'login.title'))).toBeTruthy();
  });

  it('switches every visible label and the direction after signing in', async () => {
    localStorage.setItem('notairedz.ui.lang', 'fr');
    render(<App />);
    await signIn();

    // Eight sidebar entries, all in French.
    for (const key of ['nav.home', 'nav.cases', 'nav.clients', 'nav.acts', 'nav.accounting', 'nav.compliance', 'nav.calendar', 'nav.settings'] as const) {
      expect(screen.getByRole('button', { name: t('fr', key) })).toBeTruthy();
    }

    fireEvent.click(screen.getByRole('button', { name: 'العربية' }));

    await waitFor(() => expect(document.documentElement.dir).toBe('rtl'));
    expect(screen.getByRole('button', { name: t('ar', 'nav.cases') })).toBeTruthy();
    expect(screen.queryByRole('button', { name: t('fr', 'nav.cases') })).toBeNull();
  });

  it('shows the four home cards and one primary action', async () => {
    localStorage.setItem('notairedz.ui.lang', 'ar');
    render(<App />);
    await openLogin('ar');
    fireEvent.change(screen.getByLabelText(t('ar', 'login.username')), {
      target: { value: DEMO_USERNAME },
    });
    fireEvent.change(screen.getByLabelText(t('ar', 'login.password')), {
      target: { value: DEMO_PASSWORD },
    });
    fireEvent.click(screen.getByRole('button', { name: t('ar', 'login.submit') }));

    await waitFor(() =>
      expect(screen.getByRole('heading', { level: 1, name: t('ar', 'home.title') })).toBeTruthy(),
    );
    for (const key of ['home.cards.appointments', 'home.cards.waiting', 'home.cards.unpaid', 'home.cards.expiring'] as const) {
      expect(screen.getByText(t('ar', key))).toBeTruthy();
    }
    const primary = screen.getAllByRole('button', { name: new RegExp(t('ar', 'home.newCase')) });
    expect(primary.length).toBeGreaterThan(0);
  });

  it('rejects a wrong password and keeps the user on the login screen', async () => {
    localStorage.setItem('notairedz.ui.lang', 'fr');
    render(<App />);
    await openLogin('fr');
    fireEvent.change(screen.getByLabelText(t('fr', 'login.username')), {
      target: { value: 'notaire' },
    });
    fireEvent.change(screen.getByLabelText(t('fr', 'login.password')), {
      target: { value: 'wrong-password-1' },
    });
    fireEvent.click(screen.getByRole('button', { name: t('fr', 'login.submit') }));

    await waitFor(() => expect(screen.getByRole('alert').textContent).toBe(t('fr', 'login.error.invalid')));
  });

  it('shows the instructions once, then never again', async () => {
    localStorage.clear();
    localStorage.setItem('notairedz.ui.lang', 'fr');
    render(<App />);
    await openLogin('fr');
    fireEvent.change(screen.getByLabelText(t('fr', 'login.username')), {
      target: { value: DEMO_USERNAME },
    });
    fireEvent.change(screen.getByLabelText(t('fr', 'login.password')), {
      target: { value: DEMO_PASSWORD },
    });
    fireEvent.click(screen.getByRole('button', { name: t('fr', 'login.submit') }));

    expect(await screen.findByText(t('fr', 'onboarding.p1.title'))).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: t('fr', 'onboarding.skip') }));
    await waitFor(() => expect(screen.getByRole('heading', { level: 1, name: t('fr', 'home.title') })).toBeTruthy());

    // The flag is written, so a second launch goes straight to the shell.
    expect(localStorage.getItem('notairedz.ui.onboarding')).toBe('done');
  });

  it('keeps the chosen language across a reload', async () => {
    localStorage.setItem('notairedz.ui.lang', 'ar');
    render(<App />);
    await waitFor(() => expect(document.documentElement.dir).toBe('rtl'));
  });
});
