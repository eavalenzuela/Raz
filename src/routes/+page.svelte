<script>
  import MenuButton from "$lib/components/MenuButton.svelte";
  import TabBar from "$lib/components/TabBar.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import AppsTab from "$lib/tabs/AppsTab.svelte";
  import LinksTab from "$lib/tabs/LinksTab.svelte";
  import ServersTab from "$lib/tabs/ServersTab.svelte";

  let activeTab = $state("apps");
</script>

<div class="app-shell">
  <header class="toolbar">
    <MenuButton />
  </header>

  <div class="main-area">
    <div class="content-side">
      <TabBar bind:activeTab />
      <div class="tab-panel">
        {#if activeTab === "apps"}
          <AppsTab />
        {:else if activeTab === "links"}
          <LinksTab />
        {:else if activeTab === "servers"}
          <ServersTab />
        {/if}
      </div>
    </div>
    <Sidebar />
  </div>

  <StatusBar />
</div>

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  :global(:root) {
    --bg: #f6f6f6;
    --surface: #ffffff;
    --border: #e0e0e0;
    --hover: #ebebeb;
    --text: #1a1a1a;
    --text-muted: #888888;
    --accent: #396cd8;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    font-size: 14px;
    color: var(--text);
    background: var(--bg);
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #1e1e1e;
      --surface: #2a2a2a;
      --border: #3a3a3a;
      --hover: #333333;
      --text: #e0e0e0;
      --text-muted: #888888;
      --accent: #5b8def;
    }
  }

  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
  }

  .toolbar {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-shrink: 0;
  }

  .main-area {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .content-side {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .tab-panel {
    flex: 1;
    overflow-y: auto;
  }
</style>
