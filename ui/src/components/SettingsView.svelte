<script lang="ts">
  import * as api from "../lib/api";

  interface Props {
    open?: boolean;
    theme?: "dark" | "light";
    onclose?: () => void;
    ontoggletheme?: () => void;
  }

  let { open = false, theme = "dark", onclose, ontoggletheme }: Props = $props();

  let activeTab = $state<"general" | "shortcuts" | "about">("general");
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

  /// Open the native "Save File" dialog when available (Tauri plugin-dialog /
  /// window.showSaveFilePicker), otherwise fall back to window.prompt.
  async function pickHarPath(): Promise<string | null> {
    const suggested = harDefaultFilename.trim() || "trace.har";

    // Modern browsers / WebView2 WebDriver: native save picker.
    if (typeof window.showSaveFilePicker === "function") {
      try {
        const handle = await window.showSaveFilePicker({
          suggestedName: suggested,
          types: [
            {
              description: "HAR 1.2 file",
              accept: { "application/json": [".har", ".json"] },
            },
          ],
        });
        return handle?.name ?? String(handle);
      } catch {
        // User cancelled or picker unsupported; fall through to prompt.
      }
    }

    // Fallback for environments without a file picker.
    return window.prompt("请输入导出文件路径 (例如: trace.har):", suggested) ?? null;
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
</script>

  $effect(() => {
    if (open) {
      api.loadTheme().then(theme => {
        // Handle loaded theme if needed
      });
    }
  });

  function handleThemeChange() {
    ontoggletheme?.();
  }

  function handleSave() {
    onclose?.();
  }
</script>

{#if open}
  <div class="modal-overlay" onclick={handleSave}>
    <div class="modal-content" onclick={(e) => e.stopPropagation()}>
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
            {#if import.meta.env.DEV}
            <div class="setting-row">
              <label>事件日志最大条数</label>
              <input type="number" bind:value={eventMaxCap} class="setting-input" min="100" max="50000" step="100" />
              <button class="btn btn-secondary" onclick={setEventMax}>设置</button>
            </div>
            {/if}

              <label>从持久化设置加载</label>
              <input type={usePersistentMax ? "checkbox" : "radio"} id="persistent-max" name="persistent-max" bind:checked={usePersistentMax} class="setting-input" />
              <label for="persistent-max">启用</label>

              {#if usePersistentMax}
                <span class="muted">当前加载的最大值：{loadedPersistentMax}</span>
              {/if}
            </div>

              <label>主题偏好</label>
              <select class="setting-select">
                <option value="dark">深色</option>
                <option value="light">浅色</option>
                <option value="system">跟随系统</option>
              </select>
              <label>导出 HAR</label>
              <input type="checkbox" bind:checked={showHarExportModal} />
              <span class="muted">打开导出对话框</span>

              {#if showHarExportModal}
                <div class="settings-group" style="margin-top: 1rem; padding: 1rem; background: var(--panel-bg, #1a1a1a); border-radius: 6px; border: 1px solid var(--border-color, #333);">
                  <h4 style="margin-top: 0;">导出 HAR 对话框</h4>

                  <div class="setting-row">
                    <label>导出文件路径</label>
                    <input
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
                    <label>文件名建议</label>
                    <input
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
          </div>

        {:else if activeTab === "shortcuts"}
          <div class="shortcuts-list">
            <div class="shortcut-row">
              <span class="shortcut-key">Ctrl+F</span>
              <span class="shortcut-desc">聚焦过滤器</span>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-key">↑/↓</span>
              <span class="shortcut-desc">上下选择行</span>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-key">Enter</span>
              <span class="shortcut-desc">展开详情</span>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-key">Esc</span>
              <span class="shortcut-desc">清除选择/过滤</span>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-key">Ctrl+E</span>
              <span class="shortcut-desc">导出 HAR</span>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-key">F2</span>
              <span class="shortcut-desc">切换主题</span>
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