<script lang="ts">
  import * as api from "../lib/api";
import * as rewrite from "../lib/rewriteRules";
import type {
  RewriteDraftErrors,
  RewriteRule,
  RewriteRuleDraft,
} from "../lib/rewriteRules";

  interface Props {
    open?: boolean;
    theme?: "dark" | "light";
    onclose?: () => void;
    ontoggletheme?: () => void;
  }

  let { open = false, theme = "dark", onclose, ontoggletheme }: Props = $props();

  import type { Shortcut, ShortcutCombo } from "../lib/shortcuts";
  import { shortcutRegistry, formatCombo, DEFAULT_SHORTCUTS } from "../lib/shortcuts";

  let activeTab = $state<"general" | "shortcuts" | "about" | "rewrite">("general");
  // 主题下拉框的本地选中值；通过下面的 $effect 与外部传入的 `theme` 保持同步。
  let themePreference = $state<"dark" | "light">("dark");
  $effect(() => {
    themePreference = theme;
  });
  let eventMaxCap = $state(10000);
  let usePersistentMax = $state(false); // checkbox: use persisted max from event_max.txt
  let loadedPersistentMax = $state(10000); // cached persistent max
  let showHarExportModal = $state(false);
  let harPath = $state("");
  let harDefaultFilename = $state("trace.har");
  let harIncludeHeaders = $state(true);
  let harIncludeBodies = $state(true);
  let harIncludeTiming = $state(true);
  let harExportError = $state<string | null>(null);
  let harExportSuccess = $state<string | null>(null);
  let rewriteRules = $state<RewriteRule[]>([]);
  let rewriteError = $state<string | null>(null);
  let rewriteSaving = $state(false);
  let draftMatch = $state("");
  let draftReplacement = $state("");
  let draftPriority = $state(100);
  let draftEnabled = $state(true);
  let draftErrors = $state<RewriteDraftErrors>({});
  let removingId = $state<string | null>(null);

  /// Open the native Tauri "Save File" dialog (tauri-plugin-dialog).
  ///
  /// Returns the absolute path the user picked, or `null` when they cancel.
  /// The modal keeps a manual path input as a fallback for web/dev runs.
  async function pickHarPath(): Promise<string | null> {
    const suggested = harDefaultFilename.trim() || "trace.har";

    try {
      return await api.pickSavePath(suggested);
    } catch (e) {
      console.warn("native save dialog unavailable, falling back to prompt", e);
      return window.prompt("请输入导出文件路径 (例如: trace.har):", suggested) ?? null;
    }
  }

  async function exportHar() {
    harExportError = null;
    harExportSuccess = null;

    const path = await pickHarPath();
    if (!path) return;

    try {
      await api.exportHAR(path, {
        include_headers: harIncludeHeaders,
        include_bodies: harIncludeBodies,
        include_timing: harIncludeTiming,
      });
      harExportSuccess = `已导出 HAR 到：${path}`;
      // Keep modal open briefly so the user can read the success message,
      // then close after a short delay.
      await new Promise((r) => setTimeout(r, 800));
      showHarExportModal = false;
    } catch (e) {
      harExportError = `导出 HAR 失败：${e}`;
      console.error("HAR export failed", e);
    }
  }

  async function loadPersistentMax() {
    try {
      const cap = await api.getEventMax();
      loadedPersistentMax = cap;
      eventMaxCap = cap;
      usePersistentMax = true;
    } catch {
      usePersistentMax = false;
      eventMaxCap = 10000;
    }
  }

  function setEventMax() {
    api.setEventMax(eventMaxCap).catch(() => {
      // best-effort
    });
  }

  // ----- Editable shortcuts -----

  // Re-read from the registry each render so external resets (e.g. App startup)
  // are reflected without manual copying.
  let shortcuts = $state<Shortcut[]>([]);
  let shortcutError = $state<string | null>(null);
  let capturingActionId = $state<string | null>(null);
  let savingShortcuts = $state(false);

  function refreshShortcuts() {
    shortcuts = shortcutRegistry.list();
    shortcutError = null;
  }

  $effect(() => {
    if (open) refreshShortcuts();
  });

  function captureKeydown(e: KeyboardEvent, actionId: string) {
    e.preventDefault();
    e.stopPropagation();
    // Ignore standalone modifier keys; we only record a combo once a real key is pressed.
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;

    const combo: ShortcutCombo = {
      ctrl: e.ctrlKey || e.metaKey,
      alt: e.altKey,
      shift: e.shiftKey,
      key: e.key,
    };

    // Never allow a shortcut with no action / no key.
    if (!combo.key) {
      shortcutError = "快捷键必须包含一个按键";
      return;
    }

    const conflict = shortcutRegistry.findConflict(combo, actionId);
    if (conflict) {
      shortcutError = `快捷键冲突：${formatCombo(combo)} 已被“${conflict.label}”占用`;
      return;
    }

    try {
      shortcutRegistry.set(actionId, combo);
    } catch (err) {
      shortcutError = err instanceof Error ? err.message : String(err);
      return;
    }

    shortcutError = null;
    capturingActionId = null;
    shortcuts = shortcutRegistry.list();
    persist();
  }

  function resetShortcut(actionId: string) {
    shortcutRegistry.resetOne(actionId);
    shortcutError = null;
    shortcuts = shortcutRegistry.list();
    persist();
  }

  function resetAllShortcuts() {
    shortcutRegistry.resetAll();
    shortcutError = null;
    shortcuts = shortcutRegistry.list();
    persist();
  }

  async function persist() {
    savingShortcuts = true;
    try {
      await api.saveShortcuts(shortcutRegistry.toMap());
    } catch (e) {
      shortcutError = `保存快捷键失败：${e}`;
    } finally {
      savingShortcuts = false;
    }
  }

  // ----- Rewrite rules -----

  async function refreshRewriteRules() {
    try {
      const rules = await api.listRewriteRules();
      rewriteRules = (rules ?? []).map((r) => rewrite.normalizeRule(r));
      rewriteError = null;
    } catch (e) {
      rewriteError = `加载重写规则失败：${e}`;
    }
  }

  function currentDraft(): RewriteRuleDraft {
    return {
      match: draftMatch,
      replacement: draftReplacement,
      priority: draftPriority,
      enabled: draftEnabled,
    };
  }

  async function submitRewriteRule() {
    const draft = currentDraft();
    draftErrors = rewrite.validateRewriteDraft(draft);
    if (Object.keys(draftErrors).length > 0) return;

    rewriteSaving = true;
    rewriteError = null;
    try {
      await api.addRewriteRule(rewrite.draftToRulePayload(draft));
      draftMatch = "";
      draftReplacement = "";
      draftPriority = rewrite.DEFAULT_PRIORITY;
      draftEnabled = true;
      draftErrors = {};
      await refreshRewriteRules();
    } catch (e) {
      rewriteError = `添加规则失败：${e}`;
    } finally {
      rewriteSaving = false;
    }
  }

  async function removeRewriteRule(id: string) {
    removingId = id;
    rewriteError = null;
    try {
      await api.removeRewriteRule(id);
      await refreshRewriteRules();
    } catch (e) {
      rewriteError = `删除规则失败：${e}`;
    } finally {
      removingId = null;
    }
  }

  async function toggleRewriteRule(rule: RewriteRule) {
    rewriteError = null;
    try {
      await api.toggleRewriteRule(rule.id, !rule.enabled);
      await refreshRewriteRules();
    } catch (e) {
      rewriteError = `切换规则状态失败：${e}`;
    }
  }

  $effect(() => {
    if (open) {
      api.loadTheme().then(theme => {
        // Handle loaded theme if needed
      });
    }
  });

  /// Refresh the rule list whenever the rewrite tab becomes visible, so the
  /// table always reflects the engine's current state.
  $effect(() => {
    if (open && activeTab === "rewrite") refreshRewriteRules();
  });

  function handleThemeSelect(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as "dark" | "light";
    themePreference = value;
    // `ontoggletheme` flips between the two themes; only call it when the
    // requested value differs from the currently applied theme.
    if (value !== theme) ontoggletheme?.();
  }

  function handlePersistentMaxToggle(e: Event) {
    const enabled = (e.currentTarget as HTMLInputElement).checked;
    usePersistentMax = enabled;
    if (enabled) void loadPersistentMax();
  }

  let settingsStatus = $state<string | null>(null);

  async function persistSettings() {
    try {
      await api.saveSettings({
        capture: { backend: "mock", filter: "all", auto_start: false },
        highlight_rules: [],
        responder_rules: [],
        filter_profiles: [],
        theme: themePreference,
      });
      settingsStatus = "设置已保存";
    } catch (e) {
      settingsStatus = `保存失败：${e}`;
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={handleSave} role="presentation">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal-content" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="设置" tabindex="-1">
      <div class="modal-header">
        <h2>设置</h2>
        <button class="close-btn" onclick={handleSave} aria-label="关闭">✕</button>
      </div>

      <div class="modal-tabs">
        <button
          class="tab-btn"
          class:active={activeTab === "general"}
          onclick={() => (activeTab = "general")}
        >
          通用
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === "shortcuts"}
          onclick={() => (activeTab = "shortcuts")}
        >
          快捷键
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === "rewrite"}
          onclick={() => (activeTab = "rewrite")}
        >
          重写规则
        </button>
        <button
          class="tab-btn"
          class:active={activeTab === "about"}
          onclick={() => (activeTab = "about")}
        >
          关于
        </button>
      </div>

      <div class="modal-body">
        {#if activeTab === "general"}
          <div class="settings-group">
            <h3>主题</h3>
            <div class="setting-row">
              <label for="theme-pref">主题偏好</label>
              <select
                id="theme-pref"
                class="setting-select"
                bind:value={themePreference}
                onchange={handleThemeSelect}
              >
                <option value="dark">深色</option>
                <option value="light">浅色</option>
              </select>
            </div>
          </div>

          <div class="settings-group">
            <h3>事件日志</h3>
            {#if import.meta.env.DEV}
              <div class="setting-row">
                <label for="event-max">事件日志最大条数</label>
                <input
                  id="event-max"
                  type="number"
                  class="setting-input"
                  min="100"
                  max="50000"
                  step="100"
                  bind:value={eventMaxCap}
                />
                <button class="btn btn-secondary" onclick={setEventMax}>设置</button>
              </div>
            {/if}

            <div class="setting-row">
              <label for="persistent-max">从持久化设置加载</label>
              <input
                type="checkbox"
                id="persistent-max"
                name="persistent-max"
                bind:checked={usePersistentMax}
                onchange={handlePersistentMaxToggle}
              />
              {#if usePersistentMax}
                <span class="muted">当前加载的最大值：{loadedPersistentMax}</span>
              {/if}
            </div>
          </div>

          <div class="settings-group">
            <h3>导出</h3>
            <div class="setting-row">
              <label for="har-export-toggle">导出 HAR</label>
              <input type="checkbox" id="har-export-toggle" bind:checked={showHarExportModal} />
              <span class="muted">打开导出对话框</span>
            </div>

            {#if showHarExportModal}
                <div class="settings-group" style="margin-top: 1rem; padding: 1rem; background: var(--panel-bg, #1a1a1a); border-radius: 6px; border: 1px solid var(--border-color, #333);">
                  <h4 style="margin-top: 0;">导出 HAR 对话框</h4>

                  <div class="setting-row">
                    <label for="har-path">导出文件路径</label>
                    <input
                      id="har-path"
                      type="text"
                      bind:value={harPath}
                      placeholder="trace.har"
                      class="setting-input"
                      readonly
                    />
                    <button class="btn btn-secondary" onclick={async () => {
                      const p = await pickHarPath();
                      if (p) harPath = p;
                    }}>选择文件</button>
                  </div>

                  <div class="setting-row">
                    <label for="har-filename">文件名建议</label>
                    <input
                      id="har-filename"
                      type="text"
                      bind:value={harDefaultFilename}
                      class="setting-input"
                    />
                  </div>

                  <div class="setting-row" style="display: flex; gap: 1rem; flex-wrap: wrap;">
                    <label style="margin: 0; display: flex; align-items: center; gap: 0.5rem;">
                      <input type="checkbox" bind:checked={harIncludeHeaders} class="setting-input" />
                      包含请求/响应头
                    </label>
                    <label style="margin: 0; display: flex; align-items: center; gap: 0.5rem;">
                      <input type="checkbox" bind:checked={harIncludeBodies} class="setting-input" />
                      包含请求/响应体
                    </label>
                    <label style="margin: 0; display: flex; align-items: center; gap: 0.5rem;">
                      <input type="checkbox" bind:checked={harIncludeTiming} class="setting-input" />
                      包含耗时
                    </label>
                  </div>

                  <button
                    class="btn btn-primary"
                    onclick={exportHar}
                    disabled={!harPath.trim()}
                  >
                    导出 HAR
                  </button>

                  {#if harExportError}
                    <p class="muted" style="color: var(--error-color, #ef4444); margin-top: 0.5rem;">
                      {harExportError}
                    </p>
                  {/if}
                  {#if harExportSuccess}
                    <p class="muted" style="color: var(--success-color, #22c55e); margin-top: 0.5rem; font-weight: 600;">
                      {harExportSuccess}
                    </p>
                  {/if}
                </div>
              {/if}
            </div>

          <div class="settings-group">
            <h3>持久化</h3>
            <div class="setting-row">
              <button class="btn btn-primary" onclick={persistSettings}>保存设置到磁盘</button>
              {#if settingsStatus}<span class="muted">{settingsStatus}</span>{/if}
            </div>
          </div>

        {:else if activeTab === "shortcuts"}
          <div class="shortcuts-editor">
            <div class="shortcuts-header">
              <span class="muted">点击右侧输入框后按下组合键即可重新绑定。</span>
              <button class="btn btn-secondary" onclick={resetAllShortcuts} disabled={savingShortcuts}>
                恢复默认
              </button>
            </div>

            {#if shortcutError}
              <p class="shortcut-error" role="alert">{shortcutError}</p>
            {/if}

            <div class="shortcuts-list">
              {#each shortcuts as sc (sc.actionId)}
                {@const isDefault = shortcutRegistry.isDefault(sc.actionId)}
                {@const comboTxt = formatCombo(sc.combo)}
                <div class="shortcut-row">
                  <span class="shortcut-desc">{sc.label}</span>

                  <input
                    class="shortcut-input"
                    class:capturing={capturingActionId === sc.actionId}
                    readonly
                    value={capturingActionId === sc.actionId ? "按下组合键…" : comboTxt}
                    placeholder="按下组合键…"
                    aria-label={`${sc.label} 快捷键`}
                    onfocus={() => {
                      capturingActionId = sc.actionId;
                      shortcutError = null;
                    }}
                    onblur={() => {
                      if (capturingActionId === sc.actionId) capturingActionId = null;
                    }}
                    onkeydown={(e) => captureKeydown(e, sc.actionId)}
                  />

                  <button
                    class="btn btn-secondary shortcut-reset"
                    onclick={() => resetShortcut(sc.actionId)}
                    disabled={isDefault || savingShortcuts}
                    title={isDefault ? "已是默认快捷键" : "恢复该项目默认快捷键"}
                  >
                    重置
                  </button>
                </div>
              {/each}
            </div>

            <p class="muted shortcut-hint">
              提示：默认值 {DEFAULT_SHORTCUTS.map((d) => formatCombo(d.combo)).join(" / ")}。
              改动会立即生效并保存到 shortcuts.json。
            </p>
          </div>

        {:else if activeTab === "rewrite"}
          <div class="rewrite-editor">
            <div class="rewrite-header">
              <span class="muted">
                规则按优先级从高到低执行，同优先级按添加顺序执行。
              </span>
              <span class="muted">共 {rewriteRules.length} 条规则</span>
            </div>

            {#if rewriteError}
              <p class="rewrite-error" role="alert">{rewriteError}</p>
            {/if}

            <!-- Rule list -->
            {#if rewriteRules.length === 0}
              <p class="muted rewrite-empty">暂无重写规则。</p>
            {:else}
              <table class="rewrite-table">
                <thead>
                  <tr>
                    <th>ID</th>
                    <th>匹配</th>
                    <th>替换</th>
                    <th>优先级</th>
                    <th>启用</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {#each rewriteRules as rule (rule.id)}
                    <tr class:disabled={!rule.enabled}>
                      <td class="rewrite-id" title={rule.id}>{rule.id}</td>
                      <td class="rewrite-value" title={rewrite.describeMatch(rule)}>
                        {rewrite.describeMatch(rule)}
                      </td>
                      <td class="rewrite-value" title={rewrite.describeReplacement(rule)}>
                        {rewrite.describeReplacement(rule)}
                      </td>
                      <td class="rewrite-priority">{rule.priority}</td>
                      <td>
                        <label class="rewrite-toggle">
                          <input
                            type="checkbox"
                            checked={rule.enabled}
                            onchange={() => toggleRewriteRule(rule)}
                            aria-label={`启用规则 ${rule.id}`}
                          />
                          <span>{rule.enabled ? "是" : "否"}</span>
                        </label>
                      </td>
                      <td>
                        <button
                          class="btn btn-secondary rewrite-remove"
                          onclick={() => removeRewriteRule(rule.id)}
                          disabled={removingId === rule.id}
                        >
                          {removingId === rule.id ? "删除中…" : "删除"}
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}

            <!-- Add rule form -->
            <div class="rewrite-form">
              <h4>添加规则</h4>

              <div class="rewrite-field">
                <label for="rewrite-match">匹配（URL 正则）</label>
                <input
                  id="rewrite-match"
                  type="text"
                  bind:value={draftMatch}
                  class="setting-input"
                  placeholder="https://api\\.example\\.com/v1"
                  aria-invalid={draftErrors.match ? "true" : undefined}
                />
                {#if draftErrors.match}
                  <p class="rewrite-field-error" role="alert">{draftErrors.match}</p>
                {/if}
              </div>

              <div class="rewrite-field">
                <label for="rewrite-replacement">替换（目标 URL）</label>
                <input
                  id="rewrite-replacement"
                  type="text"
                  bind:value={draftReplacement}
                  class="setting-input"
                  placeholder="https://staging.example.com/v1"
                  aria-invalid={draftErrors.replacement ? "true" : undefined}
                />
                {#if draftErrors.replacement}
                  <p class="rewrite-field-error" role="alert">{draftErrors.replacement}</p>
                {/if}
              </div>

              <div class="rewrite-field">
                <label for="rewrite-priority">优先级（{rewrite.MIN_PRIORITY}-{rewrite.MAX_PRIORITY}）</label>
                <input
                  id="rewrite-priority"
                  type="number"
                  bind:value={draftPriority}
                  class="setting-input"
                  min={rewrite.MIN_PRIORITY}
                  max={rewrite.MAX_PRIORITY}
                  step="1"
                  aria-invalid={draftErrors.priority ? "true" : undefined}
                />
                {#if draftErrors.priority}
                  <p class="rewrite-field-error" role="alert">{draftErrors.priority}</p>
                {/if}
              </div>

              <div class="rewrite-field rewrite-enabled">
                <label>
                  <input type="checkbox" bind:checked={draftEnabled} />
                  启用该规则
                </label>
              </div>

              <button
                class="btn btn-primary"
                onclick={submitRewriteRule}
                disabled={rewriteSaving}
              >
                {rewriteSaving ? "添加中…" : "添加规则"}
              </button>
            </div>
          </div>

        {:else if activeTab === "about"}
          <div class="about-section">
            <h3>FlowLens</h3>
            <p>版本：0.2.0</p>
            <p>原生 Rust HTTP/HTTPS 抓包工具</p>
            <p class="license">MIT License · © 2026 FlowLens Contributors</p>
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn btn-primary" onclick={handleSave}>保存并关闭</button>
        <button class="btn" onclick={onclose}>取消</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .rewrite-editor {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .rewrite-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    font-size: 12px;
  }

  .rewrite-error {
    margin: 0;
    padding: 6px 8px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--error-color, #ef4444);
    border: 1px solid var(--error-color, #ef4444);
    background: rgba(239, 68, 68, 0.08);
  }

  .rewrite-empty {
    font-size: 12px;
    margin: 0;
  }

  .rewrite-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    table-layout: fixed;
  }

  .rewrite-table th {
    text-align: left;
    font-weight: 600;
    color: var(--fg-muted, #9ca3af);
    border-bottom: 1px solid var(--border, #333);
    padding: 4px 6px;
  }

  .rewrite-table td {
    padding: 4px 6px;
    border-bottom: 1px solid var(--border, #333);
    vertical-align: middle;
  }

  .rewrite-table tr.disabled {
    opacity: 0.55;
  }

  .rewrite-id,
  .rewrite-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  }

  .rewrite-priority {
    text-align: right;
    font-variant-numeric: tabular-nums;
    width: 64px;
  }

  .rewrite-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }

  .rewrite-remove {
    font-size: 11px;
    padding: 3px 8px;
  }

  .rewrite-form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    border-top: 1px solid var(--border, #333);
    padding-top: 0.75rem;
  }

  .rewrite-form h4 {
    margin: 0;
    font-size: 13px;
  }

  .rewrite-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .rewrite-field label {
    font-size: 12px;
    color: var(--fg-muted, #9ca3af);
  }

  .rewrite-enabled label {
    flex-direction: row;
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg, #e5e5e5);
  }

  .rewrite-field-error {
    margin: 0;
    font-size: 11px;
    color: var(--error-color, #ef4444);
  }

  .shortcuts-editor {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .shortcuts-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .shortcuts-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border: 1px solid var(--border, #333);
    border-radius: 4px;
    background: var(--panel, #1a1a1a);
  }

  .shortcut-desc {
    flex: 1;
    font-size: 13px;
    color: var(--fg, #e5e5e5);
  }

  .shortcut-input {
    width: 160px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
    text-align: center;
    cursor: pointer;
    background: var(--bg, #111);
    color: var(--fg, #e5e5e5);
    border: 1px solid var(--border, #333);
    border-radius: 4px;
    padding: 4px 8px;
  }

  .shortcut-input.capturing {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .shortcut-reset {
    font-size: 11px;
    padding: 3px 8px;
  }

  .shortcut-error {
    margin: 0;
    padding: 6px 8px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--error-color, #ef4444);
    border: 1px solid var(--error-color, #ef4444);
    background: rgba(239, 68, 68, 0.08);
  }

  .shortcut-hint {
    font-size: 11px;
    margin: 0;
  }
</style>