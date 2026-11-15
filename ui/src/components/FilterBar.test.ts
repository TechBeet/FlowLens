// FlowLens — FilterBar component API tests.
//
// `FilterBar` exposes an imperative `focus()` (Svelte 5 `export function`)
// reached through `bind:this`. App.svelte's Ctrl+F shortcut calls
// `filterBar?.focus()`, so these tests pin down two things:
//   1. focus() targets/selects the host input (id="filter-host").
//   2. the call is a safe no-op when no instance is bound (optional chaining).
//
// jsdom is requested per-file because the suite defaults to `environment: "node"`.
// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from "vitest";
import { render, cleanup } from "@testing-library/svelte";
import FilterBar from "./FilterBar.svelte";
import { createEventStore } from "../lib/stores/eventStore.svelte";

// The store talks to Tauri (event stream + commands); stub both so the tests
// exercise only the component surface.
vi.mock("../lib/composables", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/composables")>();
  return { ...actual, subscribeTrafficEvents: () => () => {} };
});

vi.mock("../lib/api", () => ({
  getEventMax: async () => 10000,
}));

interface Focusable {
  focus: () => void;
}

let stores: Array<{ dispose: () => void }> = [];

function renderBar() {
  const store = createEventStore();
  stores.push(store);
  return render(FilterBar, { props: { store } });
}

afterEach(() => {
  cleanup();
  for (const store of stores) store.dispose();
  stores = [];
});

describe("FilterBar.focus()", () => {
  it("focuses and selects the host input", () => {
    const { container, component } = renderBar();
    const api = component as unknown as Focusable;
    const host = container.querySelector<HTMLInputElement>("#filter-host");

    expect(host).not.toBeNull();

    api.focus();

    expect(document.activeElement).toBe(host);
    // Selection covers the current value (empty by default, so 0/0).
    expect(host!.selectionStart).toBe(0);
    expect(host!.selectionEnd).toBe(host!.value.length);
  });

  it("keeps every filter label associated with its input (no a11y warnings)", () => {
    const { container } = renderBar();
    const labels = Array.from(container.querySelectorAll("label[for]"));

    expect(labels.length).toBe(5);
    for (const label of labels) {
      const target = container.querySelector(`#${label.getAttribute("for")}`);
      expect(target).not.toBeNull();
    }
  });

  it("is a no-op when FilterBar is not mounted (optional chaining)", () => {
    // `undefined as Focusable | undefined` keeps the union type from being
    // narrowed away, so this mirrors `bind:this` before mount.
    const filterBar = undefined as Focusable | undefined;

    expect(() => {
      filterBar?.focus();
    }).not.toThrow();
  });
});