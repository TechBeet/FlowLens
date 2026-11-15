import { describe, expect, it } from "vitest";
import {
  DEFAULT_PRIORITY,
  MAX_PRIORITY,
  MIN_PRIORITY,
  describeMatch,
  describeReplacement,
  draftToRulePayload,
  isValidPriority,
  newRewriteRuleId,
  normalizeRule,
  validateRewriteDraft,
  type RewriteRuleDraft,
} from "./rewriteRules";

function draft(overrides: Partial<RewriteRuleDraft> = {}): RewriteRuleDraft {
  return {
    match: "https://api\\.example\\.com/v1",
    replacement: "https://staging.example.com/v1",
    priority: 100,
    enabled: true,
    ...overrides,
  };
}

describe("isValidPriority", () => {
  it("accepts the inclusive bounds", () => {
    expect(isValidPriority(MIN_PRIORITY)).toBe(true);
    expect(isValidPriority(MAX_PRIORITY)).toBe(true);
    expect(isValidPriority(DEFAULT_PRIORITY)).toBe(true);
  });

  it("rejects out-of-range, fractional and non-numeric values", () => {
    expect(isValidPriority(-1)).toBe(false);
    expect(isValidPriority(256)).toBe(false);
    expect(isValidPriority(12.5)).toBe(false);
    expect(isValidPriority("100")).toBe(false);
    expect(isValidPriority(Number.NaN)).toBe(false);
    expect(isValidPriority(undefined)).toBe(false);
  });
});

describe("validateRewriteDraft", () => {
  it("accepts a well-formed draft", () => {
    expect(validateRewriteDraft(draft())).toEqual({});
  });

  it("rejects an empty match with an inline error", () => {
    expect(validateRewriteDraft(draft({ match: "" })).match).toBeDefined();
  });

  it("rejects a whitespace-only match", () => {
    expect(validateRewriteDraft(draft({ match: "   " })).match).toBeDefined();
  });

  it("rejects an empty replacement with an inline error", () => {
    expect(validateRewriteDraft(draft({ replacement: "" })).replacement).toBeDefined();
  });

  it("rejects a whitespace-only replacement", () => {
    expect(validateRewriteDraft(draft({ replacement: " \t " })).replacement).toBeDefined();
  });

  it("rejects priority below the range", () => {
    expect(validateRewriteDraft(draft({ priority: -1 })).priority).toBeDefined();
  });

  it("rejects priority above the range", () => {
    expect(validateRewriteDraft(draft({ priority: 256 })).priority).toBeDefined();
  });

  it("accepts the boundary priorities", () => {
    expect(validateRewriteDraft(draft({ priority: 0 }))).toEqual({});
    expect(validateRewriteDraft(draft({ priority: 255 }))).toEqual({});
  });

  it("reports every invalid field at once", () => {
    const errors = validateRewriteDraft({
      match: "",
      replacement: "",
      priority: 999,
      enabled: true,
    });
    expect(Object.keys(errors).sort()).toEqual(["match", "priority", "replacement"]);
  });

  it("rejects a syntactically invalid regex", () => {
    expect(validateRewriteDraft(draft({ match: "([unclosed" })).match).toBeDefined();
  });
});

describe("draftToRulePayload", () => {
  it("maps match to url_regex and replacement to a SetUrl action", () => {
    const payload = draftToRulePayload(draft(), "fixed-id");
    expect(payload.id).toBe("fixed-id");
    expect(payload.url_regex).toBe("https://api\\.example\\.com/v1");
    expect(payload.actions).toEqual([{ SetUrl: "https://staging.example.com/v1" }]);
  });

  it("carries priority and enabled through", () => {
    const payload = draftToRulePayload(draft({ priority: 7, enabled: false }), "id");
    expect(payload.priority).toBe(7);
    expect(payload.enabled).toBe(false);
  });

  it("trims surrounding whitespace from the fields", () => {
    const payload = draftToRulePayload(
      draft({ match: "  /api  ", replacement: "  /v2  " }),
      "id",
    );
    expect(payload.url_regex).toBe("/api");
    expect(payload.actions).toEqual([{ SetUrl: "/v2" }]);
  });

  it("defaults to the default priority constant in the draft", () => {
    expect(draftToRulePayload(draft(), "id").priority).toBe(DEFAULT_PRIORITY);
  });
});

describe("newRewriteRuleId", () => {
  it("produces distinct ids", () => {
    const ids = new Set(Array.from({ length: 50 }, () => newRewriteRuleId()));
    expect(ids.size).toBe(50);
  });

  it("is prefixed for readability", () => {
    expect(newRewriteRuleId()).toMatch(/^rule-/);
  });
});

describe("normalizeRule", () => {
  it("fills in the default priority when missing", () => {
    expect(normalizeRule({ id: "a" }).priority).toBe(DEFAULT_PRIORITY);
  });

  it("keeps an explicit priority", () => {
    expect(normalizeRule({ id: "a", priority: 3 }).priority).toBe(3);
  });

  it("replaces an out-of-range priority with the default", () => {
    expect(normalizeRule({ id: "a", priority: 1000 }).priority).toBe(DEFAULT_PRIORITY);
  });

  it("keeps a priority of zero", () => {
    expect(normalizeRule({ id: "a", priority: 0 }).priority).toBe(0);
  });
});

describe("describeMatch / describeReplacement", () => {
  it("prefers the url regex", () => {
    expect(describeMatch(normalizeRule({ url_regex: "/a" }))).toBe("/a");
  });

  it("falls back to the method regex", () => {
    expect(describeMatch(normalizeRule({ method_regex: "^POST$" }))).toBe("^POST$");
  });

  it("falls back to the header match", () => {
    expect(describeMatch(normalizeRule({ header_match: ["X-Key", "v"] }))).toBe("X-Key: v");
  });

  it("falls back to a placeholder", () => {
    expect(describeMatch(normalizeRule({}))).toBe("(全部请求)");
  });

  it("describes a SetUrl action", () => {
    expect(describeReplacement(normalizeRule({ actions: [{ SetUrl: "/new" }] }))).toBe("/new");
  });

  it("describes header actions", () => {
    expect(
      describeReplacement(normalizeRule({ actions: [{ SetHeader: ["A", "b"] }] })),
    ).toBe("A: b");
    expect(describeReplacement(normalizeRule({ actions: [{ RemoveHeader: "X" }] }))).toBe(
      "移除 X",
    );
  });

  it("returns a placeholder for an empty action list", () => {
    expect(describeReplacement(normalizeRule({}))).toBe("—");
  });
});
