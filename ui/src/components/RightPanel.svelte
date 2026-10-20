<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  
  export let event: TrafficEvent | null = null;
</script>

{#if event}
  <div class="right-panel">
    <div class="detail-header">
      <span class="detail-method">{event.method || '—'}</span>
      <span class="detail-url">{event.url || event.info || '—'}</span>
    </div>
    
    <div class="detail-tabs">
      <button class="tab-btn active">请求头</button>
      <button class="tab-btn">响应头</button>
      <button class="tab-btn">请求体</button>
      <button class="tab-btn">响应体</button>
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
  }

  .detail-header {
    padding: 12px 16px;
    border-bottom: 1px solid #222;
    background: #151520;
  }

  .detail-method {
    font-weight: 600;
    color: #7c5af3;
    font-size: 14px;
  }

  .detail-url {
    display: block;
    color: #aaa;
    font-size: 12px;
    margin-top: 4px;
    word-break: break-all;
  }

  .detail-tabs {
    display: flex;
    background: #151520;
    border-bottom: 1px solid #222;
  }

  .tab-btn {
    padding: 10px 16px;
    background: transparent;
    border: none;
    color: #888;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .tab-btn.active,
  .tab-btn:hover {
    color: #eee;
    background: #1e1e2e;
  }

  .tab-btn.active {
    color: #7c5af3;
    border-bottom: 2px solid #7c5af3;
  }

  .detail-content {
    flex: 1;
    padding: 12px;
    overflow: auto;
  }

  .event-json {
    font-size: 12px;
    line-height: 1.5;
    color: #bbb;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .empty-detail {
    color: #555;
    text-align: center;
    padding: 40px;
    font-size: 14px;
  }
</style>