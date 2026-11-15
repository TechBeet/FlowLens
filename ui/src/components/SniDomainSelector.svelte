
<script lang="ts">
  import * as api from "../lib/api";
  import type { UiDecryptEntry } from "../lib/api";
  import { onMount } from "svelte";

  let domains = $state<UiDecryptEntry[]>([]);
  let selected: string | null = $state("");
  let draft = $state("");
  let errorMsg = $state<string | null>(null);

  onMount(() => {
    refresh();
  });

  export async function refresh() {
    try {
      const resp = await api.listDecryptDomains();
      domains = resp.map((r) => ({
        domain: r.domain,
        decrypt: r.decrypt,
        wildcard: r.wildcard,
        added_at: r.added_at,
      }));
      selected = "";
      draft = "";
      errorMsg = null;
    } catch (e) {
      errorMsg = `加载域名列表失败：${e}`;
    }
  }

  async function submitDomain() {
    const value = draft.trim();
    if (value === "") {
      errorMsg = "请输入域名";
      return;
    }
    try {
      const rule = await api.addDecryptDomain(value);
      domains = [...domains, rule];
      draft = "";
      selected = rule.domain;
      errorMsg = null;
    } catch (e) {
      errorMsg = `添加域名失败：${e}`;
    }
  }

  async function removeSelected() {
    if (selected === null) {
      errorMsg = "请先选择域名";
      return;
    }
    try {
      await api.removeDecryptDomain(selected);
      domains = domains.filter((r) => r.domain !== selected);
      selected = "";
      draft = "";
      errorMsg = null;
    } catch (e) {
      errorMsg = `删除域名失败：${e}`;
    }
  }

  async function toggleDecrypt(domain: string, enable: boolean) {
    try {
      const rule = await api.toggleDecryptDomain(domain, enable);
      domains = domains.map((r) => (r.domain === rule.domain ? rule : r));
      errorMsg = null;
    } catch (e) {
      errorMsg = `更新解密开关失败：${e}`;
    }
  }
</script>

<div class="sni-dropdown">
  <input
    bind:value={draft}
    placeholder="域名（支持 *.前缀）"
  />
  <button onclick={submitDomain}>添加域名</button>
  <ul>
    {#each domains as rule (rule.domain)}
      <li>
        <button
          type="button"
          class="domain-row"
          class:selected={selected === rule.domain}
          onclick={() => {
            selected = rule.domain;
            draft = rule.domain;
          }}
        >
          <span class="domain-name">{rule.domain}</span>
          <label class="decrypt-toggle">
            <input
              type="checkbox"
              checked={rule.decrypt}
              onchange={(e) => toggleDecrypt(rule.domain, (e.target as HTMLInputElement).checked)}
            />
            <span class="toggle-label">{rule.decrypt ? "已解密" : "已加密"}</span>
          </label>
        </button>
      </li>
    {/each}
  </ul>
  {#if errorMsg}
    <span class="error">{errorMsg}</span>
  {/if}
  <button onclick={removeSelected}>删除</button>
</div>

<style>
  .sni-dropdown {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    flex-wrap: wrap;
  }
  .sni-dropdown input {
    width: 160px;
    padding: 4px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 13px;
  }
  .sni-dropdown button {
    padding: 4px 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    cursor: pointer;
  }
  .sni-dropdown ul {
    list-style: none;
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 0;
  }
  .sni-dropdown li {
    display: flex;
    align-items: center;
  }
  .domain-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    cursor: pointer;
  }
  .domain-row.selected {
    border-color: var(--accent);
    background: var(--selected);
  }
  .domain-name {
    font-family: "Consolas", monospace;
    font-size: 12px;
  }
  .decrypt-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }
  .decrypt-toggle input {
    cursor: pointer;
  }
  .error {
    color: var(--err);
    font-size: 11px;
  }
</style>