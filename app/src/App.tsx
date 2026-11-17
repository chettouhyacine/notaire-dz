import { useEffect, useMemo, useRef, useState, type JSX } from 'react';
import { applyDocumentLanguage, t, type Lang, type MessageKey } from './i18n';
import type { DigitStyle } from './lib/format';
import { demoSignIn, officeStatus, signOut, type OfficeStatus, type Principal } from './lib/api';
import { Sidebar, type NavItem, type ScreenId } from './components/Sidebar';
import { TopBar } from './components/TopBar';
import {
  IconAccounting,
  IconActs,
  IconCalendar,
  IconCases,
  IconClients,
  IconCompliance,
  IconHome,
  IconSettings,
} from './components/icons';
import { LanguageGate } from './screens/LanguageGate';
import { Login } from './screens/Login';
import { Home } from './screens/Home';
import { Clients } from './screens/Clients';
import { Cases } from './screens/Cases';
import { Settings } from './screens/Settings';
import { Setup } from './screens/Setup';
import { Welcome } from './screens/Welcome';
import { Onboarding } from './screens/Onboarding';
import { ForgotPassphrase } from './screens/ForgotPassphrase';
import { Placeholder } from './screens/Placeholder';

const NAV: readonly NavItem[] = [
  { id: 'home', labelKey: 'nav.home' },
  { id: 'cases', labelKey: 'nav.cases' },
  { id: 'clients', labelKey: 'nav.clients' },
  { id: 'acts', labelKey: 'nav.acts' },
  { id: 'accounting', labelKey: 'nav.accounting' },
  { id: 'compliance', labelKey: 'nav.compliance' },
  { id: 'calendar', labelKey: 'nav.calendar' },
  { id: 'settings', labelKey: 'nav.settings' },
];

const ICONS: Record<ScreenId, () => JSX.Element> = {
  home: () => <IconHome />,
  cases: () => <IconCases />,
  clients: () => <IconClients />,
  acts: () => <IconActs />,
  accounting: () => <IconAccounting />,
  compliance: () => <IconCompliance />,
  calendar: () => <IconCalendar />,
  settings: () => <IconSettings />,
};

const SCREEN_KEYS: Record<ScreenId, { title: MessageKey; subtitle: MessageKey }> = {
  home: { title: 'home.title', subtitle: 'home.subtitle' },
  cases: { title: 'cases.title', subtitle: 'cases.subtitle' },
  clients: { title: 'clients.title', subtitle: 'clients.subtitle' },
  acts: { title: 'acts.title', subtitle: 'acts.subtitle' },
  accounting: { title: 'accounting.title', subtitle: 'accounting.subtitle' },
  compliance: { title: 'compliance.title', subtitle: 'compliance.subtitle' },
  calendar: { title: 'calendar.title', subtitle: 'calendar.subtitle' },
  settings: { title: 'settings.title', subtitle: 'settings.subtitle' },
};

const STORAGE = {
  lang: 'notairedz.ui.lang',
  digits: 'notairedz.ui.digits',
  actLang: 'notairedz.ui.actLang',
} as const;

function readLang(key: string): Lang | null {
  try {
    const value = localStorage.getItem(key);
    return value === 'ar' || value === 'fr' ? value : null;
  } catch {
    return null;
  }
}

const ONBOARDING_KEY = 'notairedz.ui.onboarding';

/** Instructions are shown until this installation has seen them once. */
function onboardingPending(): boolean {
  try {
    return localStorage.getItem(ONBOARDING_KEY) !== 'done';
  } catch {
    return true;
  }
}

function readDigits(): DigitStyle {
  try {
    return localStorage.getItem(STORAGE.digits) === 'arabicIndic' ? 'arabicIndic' : 'western';
  } catch {
    return 'western';
  }
}

export function App(): JSX.Element {
  const [lang, setLang] = useState<Lang | null>(() => readLang(STORAGE.lang));
  const [actLang, setActLang] = useState<Lang>(() => readLang(STORAGE.actLang) ?? 'ar');
  const [digits, setDigits] = useState<DigitStyle>(readDigits);
  const [principal, setPrincipal] = useState<Principal | null>(null);
  const [screen, setScreen] = useState<ScreenId>('home');
  // `null` while the shell is still asking the core whether an office exists.
  const [office, setOffice] = useState<OfficeStatus | null>(null);
  // Signed-out screens: the welcome page, the credential form, or recovery.
  const [authStage, setAuthStage] = useState<'welcome' | 'login' | 'forgot'>('welcome');
  // Demo launches skip the sign-in screens; the attempt runs once per session.
  const demoAttempted = useRef(false);
  const [showOnboarding, setShowOnboarding] = useState(onboardingPending);

  useEffect(() => {
    let cancelled = false;
    officeStatus()
      .then((status) => {
        if (!cancelled) setOffice(status);
      })
      .catch(() => {
        if (!cancelled) {
          setOffice({
            initialized: true,
            dataDir: '',
            appVersion: 'unknown',
            recoveryAvailable: false,
            demo: false,
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Switching language updates text, direction and formatting immediately — no reload.
  useEffect(() => {
    if (lang) applyDocumentLanguage(lang);
  }, [lang]);

  useEffect(() => {
    if (!lang) return;
    try {
      localStorage.setItem(STORAGE.lang, lang);
    } catch {
      /* storage may be unavailable; the choice still applies for this session */
    }
  }, [lang]);

  useEffect(() => {
    if (!office?.demo || principal || demoAttempted.current) return;
    demoAttempted.current = true;
    let cancelled = false;
    demoSignIn()
      .then((signedIn) => {
        if (!cancelled) setPrincipal(signedIn);
      })
      .catch(() => {
        // A demo office without its key file: fall back to the normal sign-in screens.
        if (!cancelled) setAuthStage('welcome');
      });
    return () => {
      cancelled = true;
    };
  }, [office?.demo, principal]);

  const changeLanguage = (next: Lang): void => setLang(next);
  const changeDigits = (next: DigitStyle): void => {
    setDigits(next);
    try {
      localStorage.setItem(STORAGE.digits, next);
    } catch {
      /* ignore */
    }
  };
  const changeActLang = (next: Lang): void => {
    setActLang(next);
    try {
      localStorage.setItem(STORAGE.actLang, next);
    } catch {
      /* ignore */
    }
  };

  const userLabel = useMemo(() => principal?.displayName ?? '', [principal]);

  if (!lang) {
    return <LanguageGate onChoose={(chosen) => setLang(chosen)} />;
  }

  if (!office) {
    return (
      <main className="gate">
        <div className="gate-card">
          <span className="brand-mark brand-mark-large" aria-hidden="true">
            ن
          </span>
          <h1>{t(lang, 'app.name')}</h1>
          <p>{t(lang, 'loading.text')}</p>
        </div>
      </main>
    );
  }

  if (office.initialized === false && !office.demo) {
    return <Setup lang={lang} onReady={setPrincipal} />;
  }

  if (office?.demo && !principal) {
    return (
      <main className="gate">
        <div className="gate-card">
          <span className="brand-mark brand-mark-large" aria-hidden="true">
            ن
          </span>
          <h1>{t(lang, 'app.name')}</h1>
          <p>{t(lang, 'demo.opening')}</p>
        </div>
      </main>
    );
  }

  if (!principal) {
    if (authStage === 'forgot') {
      return (
        <ForgotPassphrase
          lang={lang}
          recoveryAvailable={office?.recoveryAvailable ?? true}
          onBack={() => setAuthStage('login')}
        />
      );
    }
    if (authStage === 'login') {
      return (
        <Login
          lang={lang}
          onSignedIn={setPrincipal}
          onLanguageChange={changeLanguage}
          onBack={() => setAuthStage('welcome')}
        />
      );
    }
    return (
      <Welcome
        lang={lang}
        onSignIn={() => setAuthStage('login')}
        onForgot={() => setAuthStage('forgot')}
        onLanguageChange={changeLanguage}
      />
    );
  }

  const label = (key: MessageKey): string => t(lang, key);

  if (showOnboarding) {
    return (
      <Onboarding
        lang={lang}
        onDone={() => {
          setShowOnboarding(false);
          try {
            localStorage.setItem(ONBOARDING_KEY, 'done');
          } catch {
            /* storage may be unavailable; the wizard simply shows again next launch */
          }
        }}
      />
    );
  }

  async function handleSignOut(): Promise<void> {
    await signOut();
    setPrincipal(null);
    setScreen('home');
    setAuthStage('welcome');
  }

  return (
    <div className="shell">
      <TopBar
        lang={lang}
        onLanguageChange={changeLanguage}
        userLabel={userLabel}
        demo={office?.demo ?? false}
        onSignOut={() => {
          void handleSignOut();
        }}
      />
      <div className="shell-body">
        <Sidebar
          items={NAV}
          current={screen}
          onSelect={setScreen}
          renderIcon={(id) => ICONS[id]()}
          renderLabel={label}
          ariaLabel={label('app.name')}
        />
        <main className="content">
          {screen === 'home' ? (
            <Home
              lang={lang}
              digits={digits}
              counts={{ appointments: 0, waiting: 0, unpaidCentimes: 0, expiring: 0 }}
              onNewCase={() => setScreen('cases')}
              onAccount={() => setScreen('settings')}
            />
          ) : null}

          {screen === 'clients' ? <Clients lang={lang} /> : null}

          {screen === 'cases' ? <Cases lang={lang} userId={principal.username} /> : null}

          {screen === 'settings' ? (
            <Settings
              lang={lang}
              digits={digits}
              actLang={actLang}
              username={principal.username}
              onLanguageChange={changeLanguage}
              onDigitsChange={changeDigits}
              onActLangChange={changeActLang}
              onAccountRenamed={setPrincipal}
            />
          ) : null}

          {screen !== 'home' &&
          screen !== 'settings' &&
          screen !== 'clients' &&
          screen !== 'cases' ? (
            <Placeholder
              lang={lang}
              titleKey={SCREEN_KEYS[screen].title}
              subtitleKey={SCREEN_KEYS[screen].subtitle}
            />
          ) : null}
        </main>
      </div>
      <span className="phase-badge">{label('phase.badge')}</span>
    </div>
  );
}
