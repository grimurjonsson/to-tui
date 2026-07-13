import { isKeyRelease, Key, matchesKey, type KeyId } from "@earendil-works/pi-tui";

/** Primary: ⌘⇧T (needs terminal to pass super/meta — often blocked in Cursor). */
export const TOTUI_PANEL_SHORTCUT_PRIMARY: KeyId = Key.superShift("t");

/** Fallback: works when Cmd never reaches the terminal (e.g. Cursor integrated terminal). */
export const TOTUI_PANEL_SHORTCUT_FALLBACK: KeyId = Key.ctrlShift("t");

export const TOTUI_PANEL_SHORTCUTS: readonly KeyId[] = [
	TOTUI_PANEL_SHORTCUT_PRIMARY,
	TOTUI_PANEL_SHORTCUT_FALLBACK,
];

export function matchesTotuiPanelShortcut(data: string): boolean {
	// Kitty/modifyOtherKeys send a matching release event right after press — ignore it.
	if (isKeyRelease(data)) return false;
	return TOTUI_PANEL_SHORTCUTS.some((id) => matchesKey(data, id));
}
