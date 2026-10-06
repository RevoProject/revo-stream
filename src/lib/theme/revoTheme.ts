/**
 * RevoStream look/theme contract (NEW_COLORS.md §15).
 *
 * `RevoClassic` — the existing application look, unchanged.
 * `RevoFuture`  — the new palette from plans/NEW_COLORS.md.
 *
 * The active theme lives on `<html data-theme="…">`. Values are applied by
 * static/revo-themes.css; this module is the single writer of the attribute
 * and of the localStorage cache used by the app.html bootstrap script.
 */

export type RevoTheme = "RevoClassic" | "RevoFuture";

export const REVO_THEMES: readonly RevoTheme[] = ["RevoClassic", "RevoFuture"] as const;

/* NEW_SETTINGS_UI_2 §102 — RevoFuture is the default theme. */
export const DEFAULT_REVO_THEME: RevoTheme = "RevoFuture";

export const REVO_THEME_STORAGE_KEY = "revo_theme";

export const REVO_THEME_CHANGE_EVENT = "revo:theme-change";

export const isRevoTheme = (value: unknown): value is RevoTheme =>
  value === "RevoClassic" || value === "RevoFuture";

export const normalizeRevoTheme = (value: unknown): RevoTheme =>
  isRevoTheme(value) ? value : DEFAULT_REVO_THEME;

export const readStoredRevoTheme = (): RevoTheme => {
  if (typeof window === "undefined") return DEFAULT_REVO_THEME;
  try {
    return normalizeRevoTheme(window.localStorage.getItem(REVO_THEME_STORAGE_KEY));
  } catch {
    return DEFAULT_REVO_THEME;
  }
};

/** Applies a theme to the document, persists it and notifies the current window. */
export const applyRevoTheme = (value: unknown): RevoTheme => {
  const theme = normalizeRevoTheme(value);

  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
  }

  if (typeof window !== "undefined") {
    try {
      if (window.localStorage.getItem(REVO_THEME_STORAGE_KEY) !== theme) {
        window.localStorage.setItem(REVO_THEME_STORAGE_KEY, theme);
      }
    } catch {
      // storage may be unavailable — the attribute alone is enough
    }
    window.dispatchEvent(new CustomEvent(REVO_THEME_CHANGE_EVENT, { detail: theme }));
  }

  return theme;
};

/** Reads `look.revoTheme` from a persisted ui_profile. */
export const revoThemeFromLookProfile = (look: unknown): RevoTheme => {
  const record = look && typeof look === "object" ? (look as Record<string, unknown>) : {};
  return normalizeRevoTheme(record.revoTheme);
};
