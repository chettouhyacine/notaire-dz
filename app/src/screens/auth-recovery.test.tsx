import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Welcome } from './Welcome';
import { ForgotPassphrase } from './ForgotPassphrase';
import { t } from '../i18n';

afterEach(cleanup);

describe('welcome page', () => {
  it('offers the two doors before any credential is asked for', () => {
    const onSignIn = vi.fn();
    const onForgot = vi.fn();
    render(
      <Welcome lang="ar" onSignIn={onSignIn} onForgot={onForgot} onLanguageChange={() => {}} />,
    );

    expect(screen.getByRole('heading', { level: 1, name: t('ar', 'app.name') })).toBeTruthy();
    expect(screen.queryByLabelText(t('ar', 'login.password'))).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: t('ar', 'welcome.forgot') }));
    expect(onForgot).toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: t('ar', 'welcome.signIn') }));
    expect(onSignIn).toHaveBeenCalled();
  });
});

describe('forgot passphrase', () => {
  it('asks for the recovery code and a new passphrase', () => {
    render(<ForgotPassphrase lang="fr" recoveryAvailable onBack={() => {}} />);

    expect(screen.getByLabelText(t('fr', 'forgot.code'))).toBeTruthy();
    expect(screen.getByLabelText(t('fr', 'setup.passphrase'))).toBeTruthy();
    expect(screen.getByLabelText(t('fr', 'setup.passphraseConfirm'))).toBeTruthy();
    expect(screen.getByText(t('fr', 'forgot.noCodeHint'))).toBeTruthy();
  });

  it('refuses a weak new passphrase and says which rule failed', () => {
    render(<ForgotPassphrase lang="fr" recoveryAvailable onBack={() => {}} />);
    fireEvent.change(screen.getByLabelText(t('fr', 'forgot.code')), {
      target: { value: '7K3Q-2M9X-B4TD-8JHV-6YPA-3C5R' },
    });
    fireEvent.change(screen.getByLabelText(t('fr', 'setup.passphrase')), {
      target: { value: 'court' },
    });
    fireEvent.change(screen.getByLabelText(t('fr', 'setup.passphraseConfirm')), {
      target: { value: 'court' },
    });
    fireEvent.click(screen.getByRole('button', { name: t('fr', 'forgot.submit') }));

    expect(screen.getByRole('alert').textContent).toBe(t('fr', 'setup.tooShort'));
  });

  it('says plainly that an office without a recovery code is not recoverable', () => {
    render(<ForgotPassphrase lang="ar" recoveryAvailable={false} onBack={() => {}} />);
    expect(screen.getByRole('note').textContent).toBe(t('ar', 'forgot.error.noRecovery'));
  });
});
