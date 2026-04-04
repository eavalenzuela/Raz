<script>
  let { activeTab = $bindable("apps") } = $props();

  const tabs = [
    { id: "apps", label: "Apps" },
    { id: "links", label: "Links" },
    { id: "servers", label: "Servers" },
  ];
</script>

<div class="tab-bar">
  {#each tabs as tab}
    <button
      class="tab"
      class:active={activeTab === tab.id}
      onclick={() => activeTab = tab.id}
    >
      {tab.label}
    </button>
  {/each}
</div>

<style>
  .tab-bar {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--border);
    padding: 0 8px;
  }

  .tab {
    padding: 8px 20px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    color: var(--text-muted);
    font-size: 0.9em;
    font-weight: 500;
    transition: color 0.15s, border-color 0.15s;
  }

  .tab:hover {
    color: var(--text);
  }

  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
</style>
