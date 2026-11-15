// FlowLens — shortcut registry tests.
//
// Pure logic only (no DOM): `match()` accepts any object exposing the
// KeyboardEvent modifier/key fields, so a plain literal is enough.
import { describe, it, expect } from "vitest";
import {
  ShortcutRegistry,
  DEFAULT_SHORTCUTS,
  comboString,
  formatCombo,
  toShortcutMap,
  type ShortcutCombo,
} from "./shortcuts";

function key(init: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    key: "",
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    ...init,
  } as KeyboardEvent;
}

const combo = (c: Partial<ShortcutCombo>): ShortcutCombo => ({
  ctrl: false,
  alt: false,
  shift: false,
  key: "",
  ...c,
});

describe("DEFAULT_SHORTCUTS", () => {
  it("covers the previously hard-coded actions", () => {
    const ids = DEFAULT_SHORTCUTS.map((s) => s.actionId);
    expect(ids).toEqual([
      "focus_filter",
      "toggle_capture",
      "export_har",
      "clear_selection",
      "toggle_theme",
    ]);
  });

  it("matches the documented default combos", () => {
    const map = toShortcutMap(DEFAULT_SHORTCUTS);
    expect(comboString(map.focus_filter)).toBe("ctrl+f");
    expect(comboString(map.toggle_capture)).toBe(" ");
    expect(comboString(map.export_har)).toBe("ctrl+e");
    expect(comboString(map.clear_selection)).toBe("escape");
    expect(comboString(map.toggle_theme)).toBe("f2");
  });

  it("every default has a non-empty key and label", () => {
    for (const s of DEFAULT_SHORTCUTS) {
      expect(s.combo.key.length).toBeGreaterThan(0);
      expect(s.label.length).toBeGreaterThan(0);
    }
  });
});

describe("formatCombo", () => {
  it("renders modifiers and friendly key names", () => {
    expect(formatCombo(combo({ ctrl: true, key: "f" }))).toBe("Ctrl+F");
    expect(formatCombo(combo({ key: " " }))).toBe("Space");
    expect(formatCombo(combo({ key: "Escape" }))).toBe("Esc");
    expect(formatCombo(combo({ shift: true, alt: true, key: "1" }))).toBe("Alt+Shift+1");
    expect(formatCombo(combo({ key: "F2" }))).toBe("F2");
  });
});

describe("ShortcutRegistry", () => {
  it("starts from the defaults", () => {
    const reg = new ShortcutRegistry();
    expect(reg.list()).toHaveLength(DEFAULT_SHORTCUTS.length);
    expect(reg.isDefault("focus_filter")).toBe(true);
  });

  it("detects duplicate combos", () => {
    const reg = new ShortcutRegistry();
    const conflict = reg.findConflict(combo({ ctrl: true, key: "f" }), "export_har");
    expect(conflict?.actionId).toBe("focus_filter");
  });

  it("ignores the row's own combo when validating", () => {
    const reg = new ShortcutRegistry();
    expect(reg.findConflict(combo({ ctrl: true, key: "f" }), "focus_filter")).toBeUndefined();
  });

  it("rejects a combo already used by another action", () => {
    const reg = new ShortcutRegistry();
    expect(() => reg.set("export_har", combo({ ctrl: true, key: "f" }))).toThrow(/冲突/);
    expect(comboString(reg.get("export_har")!.combo)).toBe("ctrl+e");
  });

  it("rejects a combo with no key (never actionless)", () => {
    const reg = new ShortcutRegistry();
    expect(() => reg.set("export_har", combo({ ctrl: true, key: "" }))).toThrow(/must have a key/);
    expect(comboString(reg.get("export_har")!.combo)).toBe("ctrl+e");
  });

  it("rejects unknown action ids", () => {
    const reg = new ShortcutRegistry();
    expect(() => reg.set("nope", combo({ key: "x" }))).toThrow(/unknown shortcut action/);
  });

  it("accepts a free combo and reports it as non-default", () => {
    const reg = new ShortcutRegistry();
    reg.set("export_har", combo({ ctrl: true, shift: true, key: "s" }));
    expect(comboString(reg.get("export_har")!.combo)).toBe("ctrl+shift+s");
    expect(reg.isDefault("export_har")).toBe(false);
  });

  it("resets one action and all actions", () => {
    const reg = new ShortcutRegistry();
    reg.set("export_har", combo({ ctrl: true, shift: true, key: "s" }));
    reg.resetOne("export_har");
    expect(comboString(reg.get("export_har")!.combo)).toBe("ctrl+e");

    reg.set("export_har", combo({ ctrl: true, shift: true, key: "s" }));
    reg.set("toggle_theme", combo({ ctrl: true, alt: true, key: "t" }));
    reg.resetAll();
    expect(reg.isDefault("export_har")).toBe(true);
    expect(reg.isDefault("toggle_theme")).toBe(true);
  });
});

describe("ShortcutRegistry.match", () => {
  it("resolves default combos to action ids", () => {
    const reg = new ShortcutRegistry();
    expect(reg.match(key({ key: "f", ctrlKey: true }))).toBe("focus_filter");
    expect(reg.match(key({ key: " " }))).toBe("toggle_capture");
    expect(reg.match(key({ key: "e", ctrlKey: true }))).toBe("export_har");
    expect(reg.match(key({ key: "Escape" }))).toBe("clear_selection");
    expect(reg.match(key({ key: "F2" }))).toBe("toggle_theme");
  });

  it("treats Cmd (metaKey) like Ctrl", () => {
    const reg = new ShortcutRegistry();
    expect(reg.match(key({ key: "f", metaKey: true }))).toBe("focus_filter");
  });

  it("returns null for unbound keys", () => {
    const reg = new ShortcutRegistry();
    expect(reg.match(key({ key: "f" }))).toBeNull();
    expect(reg.match(key({ key: "f", ctrlKey: true, shiftKey: true }))).toBeNull();
  });

  it("follows user overrides", () => {
    const reg = new ShortcutRegistry();
    reg.set("toggle_theme", combo({ ctrl: true, shift: true, key: "t" }));
    expect(reg.match(key({ key: "F2" }))).toBeNull();
    expect(reg.match(key({ key: "t", ctrlKey: true, shiftKey: true }))).toBe("toggle_theme");
  });
});

describe("ShortcutRegistry.merge", () => {
  it("applies persisted overrides", () => {
    const reg = new ShortcutRegistry();
    reg.merge({ focus_filter: combo({ ctrl: true, shift: true, key: "p" }) });
    expect(comboString(reg.get("focus_filter")!.combo)).toBe("ctrl+shift+p");
    expect(comboString(reg.get("export_har")!.combo)).toBe("ctrl+e");
  });

  it("resets to defaults for an empty/absent map", () => {
    const reg = new ShortcutRegistry();
    reg.set("export_har", combo({ ctrl: true, shift: true, key: "s" }));
    reg.merge({});
    expect(reg.isDefault("export_har")).toBe(true);
    reg.merge(null);
    expect(reg.isDefault("export_har")).toBe(true);
  });

  it("ignores duplicates and malformed entries in a corrupt file", () => {
    const reg = new ShortcutRegistry();
    reg.merge({
      focus_filter: combo({ ctrl: true, key: "e" }), // collides with export_har
      toggle_theme: combo({ ctrl: true, key: "" }), // missing key
      ghost_action: combo({ ctrl: true, key: "g" }), // unknown id
    } as never);
    expect(comboString(reg.get("focus_filter")!.combo)).toBe("ctrl+f");
    expect(comboString(reg.get("toggle_theme")!.combo)).toBe("f2");
    expect(reg.toMap()).not.toHaveProperty("ghost_action");
  });

  it("round-trips through toMap()/merge()", () => {
    const a = new ShortcutRegistry();
    a.set("export_har", combo({ ctrl: true, shift: true, key: "s" }));
    const b = new ShortcutRegistry();
    b.merge(a.toMap());
    expect(comboString(b.get("export_har")!.combo)).toBe("ctrl+shift+s");
  });
});
