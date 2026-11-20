// FlowLens — composable for managing rewrite rules state.
//
// Keeps the rewrite-rule editor state (draft, list, errors, loading flags)
// in one place so SettingsView stays thin and the rule logic can be tested
// independently of the UI.
import * as api from "../api";
import * as rewrite from "../rewriteRules";
import type { RewriteRule, RewriteRuleDraft, RewriteDraftErrors } from "../rewriteRules";

export interface RewriteRulesState {
  rules: RewriteRule[];
  draft: RewriteRuleDraft;
  errors: RewriteDraftErrors;
  loading: boolean;
  removingId: string | null;
  error: string | null;
}

export function useRewriteRules() {
  const state: RewriteRulesState = {
    rules: [],
    draft: {
      match: "",
      replacement: "",
      priority: rewrite.DEFAULT_PRIORITY,
      enabled: true,
    },
    errors: {},
    loading: false,
    removingId: null,
    error: null,
  };

  function setDraft(field: keyof RewriteRuleDraft, value: string | number | boolean) {
    state.draft = { ...state.draft, [field]: value };
    // Clear errors for the field being edited when it becomes valid.
    if (field === "match" || field === "replacement" || field === "priority") {
      state.errors = Object.fromEntries(
        Object.entries(state.errors).filter(([k]) => k !== field)
      );
    }
  }

  async function refresh() {
    state.loading = true;
    state.error = null;
    try {
      const rules = await api.listRewriteRules();
      state.rules = (rules ?? []).map((r) => rewrite.normalizeRule(r));
    } catch (e) {
      state.error = `加载重写规则失败：${e}`;
    } finally {
      state.loading = false;
    }
  }

  async function submit() {
    state.errors = rewrite.validateRewriteDraft(state.draft);
    if (Object.keys(state.errors).length > 0) return;

    state.loading = true;
    state.error = null;
    try {
      await api.addRewriteRule(rewrite.draftToRulePayload(state.draft));
      state.draft = {
        match: "",
        replacement: "",
        priority: rewrite.DEFAULT_PRIORITY,
        enabled: true,
      };
      state.errors = {};
      await refresh();
    } catch (e) {
      state.error = `添加规则失败：${e}`;
    } finally {
      state.loading = false;
    }
  }

  async function remove(id: string) {
    state.removingId = id;
    state.error = null;
    try {
      await api.removeRewriteRule(id);
      await refresh();
    } catch (e) {
      state.error = `删除规则失败：${e}`;
    } finally {
      state.removingId = null;
    }
  }

  async function toggle(rule: RewriteRule) {
    state.error = null;
    try {
      await api.toggleRewriteRule(rule.id, !rule.enabled);
      await refresh();
    } catch (e) {
      state.error = `切换规则状态失败：${e}`;
    }
  }

  return {
    ...state,
    setDraft,
    refresh,
    submit,
    remove,
    toggle,
  };
}
