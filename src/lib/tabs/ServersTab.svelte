<div class="tab-content">
  <div class="tab-header">
    <h2>Servers</h2>
    <button class="add-btn">+ Add Server</button>
  </div>
  <div class="empty-state">
    <p>No local servers configured yet.</p>
    <p>Click "+ Add Server" to add your first server.</p>
  </div>
</div>

<style>
  .tab-content {
    padding: 16px;
    height: 100%;
  }

  .tab-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .tab-header h2 {
    margin: 0;
    font-size: 1.2em;
  }

  .add-btn {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    font-size: 0.85em;
  }

  .add-btn:hover {
    background: var(--hover);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 50%;
    color: var(--text-muted);
    font-size: 0.95em;
  }

  .empty-state p {
    margin: 4px 0;
  }
</style>
