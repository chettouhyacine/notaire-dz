/**
 * Bridge to the Rust core.
 *
 * Inside the Tauri shell every call goes through the typed command API, where permissions and the
 * audit chain are enforced. In a plain browser (development preview only) a single hard-coded demo
 * account is accepted so the interface can be reviewed; that path cannot reach any real data
 * because the browser has no database. It is labelled in the UI as a preview.
 */
export interface Principal {
  username: string;
  roles: string[];
  displayName: string;
}

export interface OfficeStatus {
  initialized: boolean;
  dataDir: string;
  appVersion: string;
  /** False for an office created before recovery codes existed. */
  recoveryAvailable: boolean;
  /** The shell was started with `--demo`: skip sign-in and use a throwaway office. */
  demo: boolean;
}

export interface AuditReport {
  headHex: string;
  events: number;
  chainOk: boolean;
}

export interface InitPayload {
  officeName: string;
  notaryDisplayName: string;
  username: string;
  passphrase: string;
}

export const DEMO_USERNAME = 'notaire';
export const DEMO_PASSWORD = 'Correct-Horse-9';

export function isDesktopShell(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const mod = (await import('@tauri-apps/api/core')) as unknown as { invoke: Invoke };
  return (await mod.invoke(cmd, args)) as T;
}

export interface LoginOutcome {
  ok: boolean;
  principal?: Principal;
  errorKey?: 'login.error.invalid' | 'login.error.locked';
}

/**
 * In the browser preview there is no office, so setup is skipped — except with `?setup`, which
 * shows the first-run screen for review. Nothing can be created there: the command bridge is
 * absent, so the screen reports that it must be done in the desktop app.
 */
export async function officeStatus(): Promise<OfficeStatus> {
  if (!isDesktopShell()) {
    const params = typeof window !== 'undefined' ? new URLSearchParams(window.location.search) : null;
    return {
      initialized: !params?.has('setup'),
      dataDir: '',
      appVersion: 'preview',
      recoveryAvailable: true,
      demo: params?.has('demo') ?? false,
    };
  }
  const raw = await invoke<{
    initialized: boolean;
    data_dir: string;
    app_version: string;
    recovery_available: boolean;
    demo: boolean;
  }>('office_status');
  return {
    initialized: raw.initialized,
    dataDir: raw.data_dir,
    appVersion: raw.app_version,
    recoveryAvailable: raw.recovery_available,
    demo: raw.demo,
  };
}

/**
 * Demo launch: open a throwaway office without a passphrase. Only works when the desktop shell was
 * started with `--demo`; in the browser preview it just signs in as the demo user.
 */
export async function demoSignIn(): Promise<Principal> {
  if (!isDesktopShell()) {
    return { username: DEMO_USERNAME, roles: ['notary_titular'], displayName: DEMO_USERNAME };
  }
  const raw = await invoke<{ username: string; roles: string[]; display_name: string }>(
    'demo_sign_in',
  );
  return { username: raw.username, roles: raw.roles, displayName: raw.display_name };
}

export interface InitResult {
  principal: Principal;
  /** Issued once: the only way back in if the passphrase is forgotten. */
  recoveryCode: string;
}

export async function initializeOffice(payload: InitPayload): Promise<InitResult> {
  const raw = await invoke<{
    principal: { username: string; roles: string[]; display_name: string };
    recovery_code: string;
  }>('office_initialize', {
    payload: {
      office_name: payload.officeName,
      notary_display_name: payload.notaryDisplayName,
      username: payload.username,
      passphrase: payload.passphrase,
    },
  });
  return {
    principal: {
      username: raw.principal.username,
      roles: raw.principal.roles,
      displayName: raw.principal.display_name,
    },
    recoveryCode: raw.recovery_code,
  };
}

/**
 * Reset a forgotten passphrase with the recovery code. Returns a fresh code, because the one used
 * is replaced. Runs before sign-in, so no session is involved.
 */
export async function resetPassphrase(
  username: string,
  recoveryCode: string,
  newPassphrase: string,
): Promise<string> {
  const raw = await invoke<{ recovery_code: string }>('auth_reset_passphrase', {
    username,
    recoveryCode,
    newPassphrase,
  });
  return raw.recovery_code;
}

export async function signIn(username: string, password: string): Promise<LoginOutcome> {
  if (isDesktopShell()) {
    try {
      const raw = await invoke<{ username: string; roles: string[]; display_name: string }>(
        'auth_login',
        { username, password },
      );
      return {
        ok: true,
        principal: { username: raw.username, roles: raw.roles, displayName: raw.display_name },
      };
    } catch (error) {
      const code = String((error as { code?: string })?.code ?? error);
      return {
        ok: false,
        errorKey: code.includes('locked_out') ? 'login.error.locked' : 'login.error.invalid',
      };
    }
  }

  // Development preview: no database is reachable from the browser, so only the demo account works.
  if (username.trim().toLowerCase() === DEMO_USERNAME && password === DEMO_PASSWORD) {
    return {
      ok: true,
      principal: { username: DEMO_USERNAME, roles: ['notary_titular'], displayName: DEMO_USERNAME },
    };
  }
  return { ok: false, errorKey: 'login.error.invalid' };
}

export async function renameAccount(newUsername: string): Promise<Principal> {
  const raw = await invoke<{ username: string; roles: string[]; display_name: string }>(
    'account_rename',
    { newUsername },
  );
  return { username: raw.username, roles: raw.roles, displayName: raw.display_name };
}

/** Changes the passphrase that opens the office; the current one is required. */
export async function changePassword(current: string, next: string): Promise<void> {
  await invoke('account_change_password', { currentPassword: current, newPassword: next });
}

export async function signOut(): Promise<void> {
  if (isDesktopShell()) {
    await invoke('auth_logout');
  }
}

export async function auditHead(): Promise<AuditReport | null> {
  if (!isDesktopShell()) return null;
  const raw = await invoke<{ head_hex: string; events: number; chain_ok: boolean }>('audit_head');
  return { headHex: raw.head_hex, events: raw.events, chainOk: raw.chain_ok };
}

export interface PartyDto {
  id: string;
  kind: string;
  displayAr: string;
  displayLatin: string;
}

export interface CaseDto {
  id: string;
  reference: string;
  actType: string;
  stage: string;
  openedAtMs: number;
  dueAtMs: number | null;
  confidential: boolean;
}

export interface PersonPayload {
  surnameLatin: string;
  givenLatin: string;
  surnameAr: string;
  givenAr: string;
  birthDate: string;
  nin: string;
}

export interface CasePayload {
  actType: string;
  ownerUserId: string;
  dueInDays: number | null;
  confidential: boolean;
}

type RawParty = { id: string; kind: string; display_ar: string; display_latin: string };
type RawCase = {
  id: string;
  reference: string;
  act_type: string;
  stage: string;
  opened_at_ms: number;
  due_at_ms: number | null;
  confidential: boolean;
};

const toParty = (raw: RawParty): PartyDto => ({
  id: raw.id,
  kind: raw.kind,
  displayAr: raw.display_ar,
  displayLatin: raw.display_latin,
});

const toCase = (raw: RawCase): CaseDto => ({
  id: raw.id,
  reference: raw.reference,
  actType: raw.act_type,
  stage: raw.stage,
  openedAtMs: raw.opened_at_ms,
  dueAtMs: raw.due_at_ms,
  confidential: raw.confidential,
});

export async function listParties(limit = 100): Promise<PartyDto[]> {
  if (!isDesktopShell()) return [];
  const raw = await invoke<RawParty[]>('party_list', { limit });
  return raw.map(toParty);
}

export async function createPerson(payload: PersonPayload): Promise<string> {
  return invoke<string>('party_create', {
    payload: {
      surname_ar: payload.surnameAr,
      given_ar: payload.givenAr,
      surname_latin: payload.surnameLatin,
      given_latin: payload.givenLatin,
      father_name: null,
      mother_name: null,
      birth_date: payload.birthDate || null,
      nin: payload.nin || null,
    },
  });
}

export async function searchParties(query: string): Promise<PartyDto[]> {
  if (!isDesktopShell()) return [];
  const trimmed = query.trim();
  if (!trimmed) return listParties();
  // One box: the first token is treated as the surname, the rest as given names.
  const [surname, ...rest] = trimmed.split(/\s+/);
  const raw = await invoke<RawParty[]>('party_search', {
    surname,
    given: rest.join(' ') || null,
  });
  return raw.map(toParty);
}

export async function listCases(): Promise<CaseDto[]> {
  if (!isDesktopShell()) return [];
  const raw = await invoke<RawCase[]>('case_list');
  return raw.map(toCase);
}

export async function createCase(payload: CasePayload): Promise<string> {
  return invoke<string>('case_create', {
    payload: {
      act_type: payload.actType,
      owner_user_id: payload.ownerUserId,
      due_in_days: payload.dueInDays,
      confidential: payload.confidential,
    },
  });
}

export async function createBackup(passphrase: string): Promise<boolean> {
  if (!isDesktopShell()) return false;
  await invoke('backup_create', { passphrase });
  return true;
}
