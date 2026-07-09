import { getConfig, setThemeSetting } from "../persistence/api";

export type Theme = "light" | "dark";

/**
 * Theme override. Setting `data-theme` on the root element flips the CSS
 * variables (see global.css); clearing it falls back to the system preference.
 * The choice persists in yap's config so a new window opens the same way.
 */
export function applyTheme(theme: Theme | null): void {
  const root = document.documentElement;
  if (theme) root.dataset.theme = theme;
  else delete root.dataset.theme;
}

/** Read the persisted override and apply it. Called once at startup. */
export async function initTheme(): Promise<void> {
  try {
    const { theme } = await getConfig();
    applyTheme(theme === "light" || theme === "dark" ? theme : null);
  } catch {
    // No config yet, or the backend is unavailable: follow the system.
  }
}

/** Apply and persist a new override. */
export async function setTheme(theme: Theme): Promise<void> {
  applyTheme(theme);
  await setThemeSetting(theme);
}
