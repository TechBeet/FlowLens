<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  
  let { event = $bindable<TrafficEvent | null>(null) } = $props();
  
  let activeTab = $state(0);
  const tabs = ["请求头", "响应头", "请求体", "响应体"];
</script>

{#if event}
  <div class="right-panel">
    <div class="detail-header">
      <span class="detail-method">{event.method || '—'}</span>
      <span class="detail-url">{event.url || event.info || '—'}</span>
    </div>
    
    <div class="detail-tabs">
      {#each tabs as tab, i}
        <button
          class="tab-btn"
          class:active={activeTab === i}
          onclick={() => (activeTab = i)}
        >
          {tab}
        </button>
      {/each}
    </div>
    
    <div class="detail-content">
      <pre class="event-json">{JSON.stringify(event, null, 2)}</pre>
    </div>
  </div>
{:else}
  <div class="right-panel">
    <div class="empty-detail">
      请从左侧列表选择一个事件查看详情
    </div>
  </div>
{/if}

<style>
  .right-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--surface);
  }

  .detail-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .detail-method {
    font-weight: 600;
    color: var(--accent);
    font-size: 14px;
  }

  .detail-url {
    display: block;
    color: var(--fg-dim);
    font-size: 12px;
    margin-top: 4px;
    word-break: break-all;
  }

  .detail-tabs {
    display: flex;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }

  .tab-btn {
    padding: 10px 16px;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .tab-btn.active,
  .tab-btn:hover {
    color: var(--fg);
    background: var(--hover);
  }

  .tab-btn.active {
    color: var(--accent);
    border-bottom: 2px solid var(--accent);
  }

  .detail-content {
    flex: 1;
    padding: 12px;
    overflow: auto;
  }

  .event-json {
    font-size: 12px;
    line-height: 1.5;
    color: var(--fg);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .empty-detail {
    color: var(--fg-dim);
    text-align: center;
    padding: 40px;
    font-size: 14px;
  }
</style>
