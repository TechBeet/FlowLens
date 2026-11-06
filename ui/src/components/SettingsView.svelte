<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import * as api from "../lib/api";

  interface Props {
    open?: boolean;
    theme?: "dark" | "light";
    onclose?: () => void;
    ontoggletheme?: () => void;
  }

  let { open = false, theme = "dark", onclose, ontoggletheme }: Props = $props();

  let activeTab = $state<"general" | "shortcuts" | "about">("general");

  $effect(() => {
    if (open) {
      api.loadTheme();
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
              <label>主题偏好</label>
              <select class="setting-select">
                <option value="dark">深色</option>
                <option value="light">浅色</option>
                <option value="system">跟随系统</option>
              </select>
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