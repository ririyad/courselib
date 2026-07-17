export type ThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'courselib-theme';

export function getStoredMode(): ThemeMode {
  if (typeof localStorage === 'undefined') return 'system';
  const value = localStorage.getItem(STORAGE_KEY);
  return value === 'light' || value === 'dark' ? value : 'system';
}

export function systemPrefersDark(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
  );
}

export function resolveTheme(mode: ThemeMode): 'light' | 'dark' {
  return mode === 'system' ? (systemPrefersDark() ? 'dark' : 'light') : mode;
}

/**
 * Persist and apply a theme mode. "system" clears the stored preference and
 * the data-theme attribute so the CSS `prefers-color-scheme` fallback decides.
 */
export function applyMode(mode: ThemeMode): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  try {
    if (mode === 'system') {
      root.removeAttribute('data-theme');
      localStorage.removeItem(STORAGE_KEY);
    } else {
      root.setAttribute('data-theme', mode);
      localStorage.setItem(STORAGE_KEY, mode);
    }
  } catch {
    // Ignore storage failures (e.g. privacy mode); the attribute still applies.
    if (mode === 'system') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', mode);
  }
}

const CYCLE: ThemeMode[] = ['light', 'dark', 'system'];

export function nextMode(mode: ThemeMode): ThemeMode {
  const index = CYCLE.indexOf(mode);
  return CYCLE[(index + 1) % CYCLE.length];
}
