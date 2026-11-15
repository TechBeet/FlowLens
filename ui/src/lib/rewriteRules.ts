//! Rewrite-rule domain types + validation shared by the settings UI.
//!
//! The shapes here mirror `flowlens_core::RewriteRule` (see
//! `crates/flowlens-core/src/rewrite.rs`). The settings UI only exposes a
//! "match URL → replacement URL" subset of the backend model, so helpers in
//! this module translate between the compact form state and the full payload.

/** Priority used when a rule does not specify one (mirrors `DEFAULT_PRIORITY`). */
export const DEFAULT_PRIORITY = 100;
/** Inclusive lower bound for a rule priority. */
export const MIN_PRIORITY = 0;
/** Inclusive upper bound for a rule priority (`u8`). */
export const MAX_PRIORITY = 255;

/** A single rewrite action, serialised the way serde represents the Rust enum. */
export type RewriteAction =
  | { SetUrl: string }
  | { SetMethod: string }
  | { SetHeader: [string, string] }
  | { AddHeader: [string, string] }
  | { RemoveHeader: string }
  | { SetBody: string };

/** Mirrors `flowlens_core::RewriteRule`. */
export interface RewriteRule {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  method_regex: string | null;
  url_regex: string | null;
  status_code: number | null;
  header_match: [string, string] | null;
  body_contains: string | null;
  actions: RewriteAction[];
}

/** Compact form state used by the "add rule" editor. */
export interface RewriteRuleDraft {
  match: string;
  replacement: string;
  priority: number;
  enabled: boolean;
}

export type RewriteDraftErrors = Partial<
  Record<"match" | "replacement" | "priority", string>
>;

/** Generate a collision-resistant id without depending on `crypto.randomUUID`. */
export function newRewriteRuleId(now: number = Date.now()): string {
  const jitter = Math.random().toString(36).slice(2, 8);
  return `rule-${now.toString(36)}-${jitter}`;
}

/** True when `value` is an integer inside the legal `u8` priority range. */
export function isValidPriority(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= MIN_PRIORITY &&
    value <= MAX_PRIORITY
  );
}

/**
 * Validate a draft before it is sent to the backend.
 *
 * Returns an object keyed by field; an empty object means the draft is valid.
 * The match pattern is additionally compiled so an invalid regex is reported
 * inline instead of surfacing later as a silent non-match.
 */
export function validateRewriteDraft(draft: RewriteRuleDraft): RewriteDraftErrors {
  const errors: RewriteDraftErrors = {};

  const match = draft.match.trim();
  if (!match) {
    errors.match = "匹配规则不能为空";
  } else if (!isValidRegex(match)) {
    errors.match = "匹配规则不是合法的正则表达式";
  }

  if (!draft.replacement.trim()) {
    errors.replacement = "替换值不能为空";
  }

  if (!isValidPriority(draft.priority)) {
    errors.priority = `优先级必须是 ${MIN_PRIORITY}-${MAX_PRIORITY} 之间的整数`;
  }

  return errors;
}

/** Best-effort regex validation; the backend uses `regex_lite`, JS is stricter. */
function isValidRegex(pattern: string): boolean {
  try {
    // eslint-disable-next-line no-new
    new RegExp(pattern);
    return true;
  } catch {
    return false;
  }
}

/**
 * Build the backend payload for a draft.
 *
 * `match` maps to `url_regex` and `replacement` becomes a `SetUrl` action,
 * which is the subset the settings UI exposes.
 */
export function draftToRulePayload(
  draft: RewriteRuleDraft,
  id: string = newRewriteRuleId(),
): RewriteRule {
  const match = draft.match.trim();
  return {
    id,
    name: match,
    enabled: draft.enabled,
    priority: draft.priority,
    method_regex: null,
    url_regex: match,
    status_code: null,
    header_match: null,
    body_contains: null,
    actions: [{ SetUrl: draft.replacement.trim() }],
  };
}

/** Human-readable match value for the rule list. */
export function describeMatch(rule: RewriteRule): string {
  if (rule.url_regex) return rule.url_regex;
  if (rule.method_regex) return rule.method_regex;
  if (rule.header_match) return `${rule.header_match[0]}: ${rule.header_match[1]}`;
  return "(全部请求)";
}

/** Human-readable replacement value for the rule list. */
export function describeReplacement(rule: RewriteRule): string {
  const first = rule.actions?.[0];
  if (!first) return "—";
  if ("SetUrl" in first) return first.SetUrl;
  if ("SetMethod" in first) return first.SetMethod;
  if ("SetHeader" in first) return `${first.SetHeader[0]}: ${first.SetHeader[1]}`;
  if ("AddHeader" in first) return `${first.AddHeader[0]}: ${first.AddHeader[1]}`;
  if ("RemoveHeader" in first) return `移除 ${first.RemoveHeader}`;
  if ("SetBody" in first) return `设置请求体 (${first.SetBody.length} 字节)`;
  return "—";
}

/** Normalise a rule coming from the backend, defaulting a missing priority. */
export function normalizeRule(raw: Partial<RewriteRule> & { id?: string }): RewriteRule {
  return {
    id: raw.id ?? newRewriteRuleId(),
    name: raw.name ?? "",
    enabled: raw.enabled ?? true,
    priority: isValidPriority(raw.priority) ? raw.priority : DEFAULT_PRIORITY,
    method_regex: raw.method_regex ?? null,
    url_regex: raw.url_regex ?? null,
    status_code: raw.status_code ?? null,
    header_match: raw.header_match ?? null,
    body_contains: raw.body_contains ?? null,
    actions: raw.actions ?? [],
  };
}
