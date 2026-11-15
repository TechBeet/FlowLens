// FlowLens — Typed keyboard-shortcut registry.
//
// Actions are identified by a stable `actionId`. Each shortcut holds the current
// key combo (modifiers + key) plus a human-readable label.
//
// `DEFAULT_SHORTCUTS` matches the behavior that was hard-coded / documented in
// the UI before this feature existed:
//   - Ctrl+F : focus the filter input
//   - Space  : pause / resume capture
//   - Ctrl+E : export HAR
//   - Escape : clear selection / filter
//   - F2     : toggle theme
//
// User overrides are persisted through the Tauri commands `read_shortcuts` /
// `write_shortcuts` (see src-tauri/src/commands.rs and lib/api.ts).

export interface ShortcutCombo {
  /** Ctrl (or Cmd on macOS) must be held. */
  ctrl: boolean;
  /** Alt must be held. */
  alt: boolean;
  /** Shift must be held. */
  shift: boolean;
  /** DOM `KeyboardEvent.key` value, e.g. "f", " ", "Escape", "F2". */
  key: string;
}

export interface Shortcut {
  actionId: string;
  label: string;
  combo: ShortcutCombo;
}

/** Canonical string used for conflict detection and lookups. */
export function comboString(combo: ShortcutCombo): string {
  const parts: string[] = [];
  if (combo.ctrl) parts.push("ctrl");
  if (combo.alt) parts.push("alt");
  if (combo.shift) parts.push("shift");
  parts.push(combo.key.toLowerCase());
  return parts.join("+");
}

/** Human-readable rendering, e.g. "Ctrl+F" / "Shift+Alt+X" / "Space". */
export function formatCombo(combo: ShortcutCombo): string {
  const parts: string[] = [];
  if (combo.ctrl) parts.push("Ctrl");
  if (combo.alt) parts.push("Alt");
  if (combo.shift) parts.push("Shift");
  const key = combo.key;
  let label = key;
  if (key === " ") label = "Space";
  else if (key.length === 1) label = key.toUpperCase();
  else {
    // "Escape" -> "Esc", "ArrowUp" -> "↑", "F2" stays "F2"
    const map: Record<string, string> = {
      Escape: "Esc",
      ArrowUp: "↑",
      ArrowDown: "↓",
      ArrowLeft: "←",
      ArrowRight: "→",
      Enter: "Enter",
      Tab: "Tab",
      Backspace: "Backspace",
      Delete: "Delete",
    };
    label = map[key] ?? key;
  }
  parts.push(label);
  return parts.join("+");
}

/** Default shortcuts, matching the pre-existing hard-coded behavior. */
export const DEFAULT_SHORTCUTS: readonly Shortcut[] = [
  {
    actionId: "focus_filter",
    label: "聚焦过滤器",
    combo: { ctrl: true, alt: false, shift: false, key: "f" },
  },
  {
    actionId: "toggle_capture",
    label: "暂停/恢复捕获",
    combo: { ctrl: false, alt: false, shift: false, key: " " },
  },
  {
    actionId: "export_har",
    label: "导出 HAR",
    combo: { ctrl: true, alt: false, shift: false, key: "e" },
  },
  {
    actionId: "clear_selection",
    label: "清除选择/过滤",
    combo: { ctrl: false, alt: false, shift: false, key: "Escape" },
  },
  {
    actionId: "toggle_theme",
    label: "切换主题",
    combo: { ctrl: false, alt: false, shift: false, key: "F2" },
  },
] as const;

export type ShortcutMap = Record<string, ShortcutCombo>;

export interface ComboConflict {
  actionId: string;
  label: string;
}

/** Build a plain (JSON-serializable) map of overrides. */
export function toShortcutMap(shortcuts: Iterable<Shortcut>): ShortcutMap {
  const out: ShortcutMap = {};
  for (const s of shortcuts) out[s.actionId] = { ...s.combo };
  return out;
}

/** An always-editable, validated collection of shortcuts. */
export class ShortcutRegistry {
  private entries = new Map<string, Shortcut>();

  constructor(shortcuts: Shortcut[] = DEFAULT_SHORTCUTS.map((s) => ({ ...s, combo: { ...s.combo } }))) {
    for (const s of shortcuts) {
      if (!s.actionId) throw new Error("shortcut actionId must not be empty");
      if (!s.combo.key) throw new Error(`shortcut "${s.actionId}" must have a key`);
      this.entries.set(s.actionId, { ...s, combo: { ...s.combo } });
    }
    this.assertNoDuplicates();
  }

  /** All shortcuts in declaration order. */
  list(): Shortcut[] {
    return [...this.entries.values()].map((s) => ({ ...s, combo: { ...s.combo } }));
  }

  get(actionId: string): Shortcut | undefined {
    const s = this.entries.get(actionId);
    return s ? { ...s, combo: { ...s.combo } } : undefined;
  }

  /**
   * Find a shortcut conflicting with `combo`. Returns `undefined` when the combo
   * is free. `excludeActionId` lets a row re-apply its own current combo.
   */
  findConflict(combo: ShortcutCombo, excludeActionId?: string): ComboConflict | undefined {
    const target = comboString(combo);
    for (const s of this.entries.values()) {
      if (s.actionId === excludeActionId) continue;
      if (comboString(s.combo) === target) {
        return { actionId: s.actionId, label: s.label };
      }
    }
    return undefined;
  }

  /**
   * Assign a combo. Throws on conflict or when the combo has no key, so a
   * shortcut can never end up actionless or duplicated.
   */
  set(actionId: string, combo: ShortcutCombo): void {
    const entry = this.entries.get(actionId);
    if (!entry) throw new Error(`unknown shortcut action: ${actionId}`);
    if (!combo.key) throw new Error(`shortcut "${actionId}" must have a key`);
    const conflict = this.findConflict(combo, actionId);
    if (conflict) {
      throw new Error(`快捷键冲突：${formatCombo(combo)} 已被“${conflict.label}”占用`);
    }
    entry.combo = { ...combo };
  }

  resetOne(actionId: string): void {
    const def = DEFAULT_SHORTCUTS.find((s) => s.actionId === actionId);
    const entry = this.entries.get(actionId);
    if (!def || !entry) return;
    const conflict = this.findConflict(def.combo, actionId);
    if (conflict) return; // default collides with a custom binding: leave as-is
    entry.combo = { ...def.combo };
  }

  resetAll(): void {
    this.entries.clear();
    for (const s of DEFAULT_SHORTCUTS) {
      this.entries.set(s.actionId, { ...s, combo: { ...s.combo } });
    }
  }

  isDefault(actionId: string): boolean {
    const def = DEFAULT_SHORTCUTS.find((s) => s.actionId === actionId);
    const entry = this.entries.get(actionId);
    if (!def || !entry) return true;
    return comboString(def.combo) === comboString(entry.combo);
  }

  /** Merge persisted overrides on top of the defaults; unknown ids are ignored. */
  merge(saved: ShortcutMap | null | undefined): void {
    this.resetAll();
    if (!saved) return;
    const pending: Array<[string, ShortcutCombo]> = [];
    for (const def of DEFAULT_SHORTCUTS) {
      const combo = saved[def.actionId];
      if (combo && typeof combo.key === "string" && combo.key) {
        pending.push([
          def.actionId,
          { ctrl: !!combo.ctrl, alt: !!combo.alt, shift: !!combo.shift, key: combo.key },
        ]);
      }
    }
    // Apply only non-conflicting overrides; a corrupt file must not break startup.
    for (const [actionId, combo] of pending) {
      if (!this.findConflict(combo, actionId)) {
        const entry = this.entries.get(actionId);
        if (entry) entry.combo = combo;
      }
    }
    this.assertNoDuplicates();
  }

  toMap(): ShortcutMap {
    return toShortcutMap(this.entries.values());
  }

  /** Resolve a keyboard event to an action id, or `null` when unbound. */
  match(event: KeyboardEvent): string | null {
    const combo: ShortcutCombo = {
      ctrl: event.ctrlKey || event.metaKey,
      alt: event.altKey,
      shift: event.shiftKey,
      key: event.key,
    };
    const target = comboString(combo);
    for (const s of this.entries.values()) {
      if (comboString(s.combo) === target) return s.actionId;
    }
    return null;
  }

  private assertNoDuplicates(): void {
    const seen = new Set<string>();
    for (const s of this.entries.values()) {
      const cs = comboString(s.combo);
      if (seen.has(cs)) {
        throw new Error(`duplicate shortcut combo: ${formatCombo(s.combo)}`);
      }
      seen.add(cs);
    }
  }
}

/** Shared app-wide registry instance. */
export const shortcutRegistry = new ShortcutRegistry();
