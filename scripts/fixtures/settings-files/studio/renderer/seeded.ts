// Seeded for `check-settings-have-files.mjs`. Deliberately wrong: a renderer
// that keeps a choice in browser storage has put a setting where no file can be
// edited and no other process can read it.

export function rememberPanelWidth(px: number): void {
  localStorage.setItem("panelWidth", String(px));
}

export function lastOpenView(): string | null {
  return window.sessionStorage.getItem("view");
}

export function openCache(): IDBOpenDBRequest {
  return indexedDB.open("studio-cache", 1);
}

// The escape, with a reason: a name in prose is not a store.
// localStorage is the API this gate refuses. settings-allow: prose about the rule, not a use of it

// The escape with nothing after the colon, which is itself reported - otherwise
// the marker would be an off switch rather than a reviewed exception.
export const STORE_NOTE = "localStorage"; // settings-allow:

// Not one of the three names, and not reported: an identifier that merely
// starts with one of them. The match is whole-word, so `Shim` breaks it.
export const localStorageShim = { get: () => null };
