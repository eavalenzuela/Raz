<div class="status-bar">
  <span class="status-indicator online"></span>
  <span>All systems online</span>
  <span class="spacer"></span>
  <span class="timestamp">No checks yet</span>
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    border-top: 1px solid var(--border);
    font-size: 0.8em;
    color: var(--text-muted);
    background: var(--surface);
    flex-shrink: 0;
  }

  .status-indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-indicator.online {
    background: #22c55e;
  }

  .status-indicator.offline {
    background: #ef4444;
  }

  .status-indicator.unknown {
    background: #a3a3a3;
  }

  .spacer {
    flex: 1;
  }
</style>
