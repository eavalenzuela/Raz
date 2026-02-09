const data = window.__DASHBOARD_DATA__;

const state = {
  tiles: [...data.tiles],
  devices: [...data.devices],
  services: [...data.services],
  alerts: {
    debounce_seconds: data.alerts?.debounce_seconds ?? 300,
    email: { ...(data.alerts?.email || { enabled: false, target: "" }) },
    webhook: { ...(data.alerts?.webhook || { enabled: false, target: "" }) },
    ntfy: { ...(data.alerts?.ntfy || { enabled: false, target: "" }) },
    slack: { ...(data.alerts?.slack || { enabled: false, target: "" }) },
    discord: { ...(data.alerts?.discord || { enabled: false, target: "" }) },
  },
  tileRefreshHours: data.tile_refresh_hours,
  history: {
    devices: new Map(),
    services: new Map(),
  },
};

const tilesContainer = document.getElementById("tiles");
const tileGroupsContainer = document.getElementById("tile-groups");
const devicesContainer = document.getElementById("devices");
const servicesContainer = document.getElementById("services");
const devicesUpdated = document.getElementById("devices-updated");
const servicesUpdated = document.getElementById("services-updated");

const editor = document.getElementById("editor");
const editorTitle = document.getElementById("editor-title");
const editorText = document.getElementById("editor-text");
const editorSave = document.getElementById("editor-save");
const editorClose = document.getElementById("editor-close");
const editorErrors = document.getElementById("editor-errors");
const editorStatus = document.getElementById("editor-status");

let currentEdit = null;

let editorStatusTimer = null;

const clearEditorStatus = () => {
  if (editorStatusTimer) {
    clearTimeout(editorStatusTimer);
    editorStatusTimer = null;
  }
  editorStatus.textContent = "";
  editorStatus.className = "editor-status";
};

const showEditorStatus = (message, kind = "info", { autoClearMs = null } = {}) => {
  clearEditorStatus();
  editorStatus.textContent = message;
  editorStatus.classList.add("active");
  if (kind) {
    editorStatus.classList.add(`is-${kind}`);
  }
  if (autoClearMs) {
    editorStatusTimer = setTimeout(() => {
      clearEditorStatus();
    }, autoClearMs);
  }
};

const VALID_METHODS = new Set(["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]);
const ALERT_CHANNELS = ["email", "webhook", "ntfy", "slack", "discord"];

const thumbnailUrl = (tile) =>
  tile.preview || `https://image.thum.io/get/width/800/${encodeURIComponent(tile.url)}`;

const tileDisplayTitle = (tile) => tile.display_title?.trim() || tile.title;

const tileDragState = {
  dragIndex: null,
};

const getTileGroupLabel = (tile) => (tile.group || "Ungrouped").trim() || "Ungrouped";

const moveTile = (fromIndex, toIndex) => {
  if (fromIndex === toIndex || fromIndex < 0 || toIndex < 0) return;
  const [moved] = state.tiles.splice(fromIndex, 1);
  if (!moved) return;
  state.tiles.splice(toIndex, 0, moved);
};

const persistConfig = async () => {
  const response = await fetch("/api/config", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      tiles: state.tiles,
      devices: state.devices,
      services: state.services,
      alerts: state.alerts,
    }),
  });
  if (!response.ok) {
    throw new Error(`Config save failed with status ${response.status}`);
  }
  const updated = await response.json();
  state.tiles = updated.tiles;
  state.devices = updated.devices;
  state.services = updated.services;
  state.alerts = updated.alerts;
};

const makeTileDraggable = (tileEl, index) => {
  tileEl.draggable = true;
  tileEl.dataset.tileIndex = String(index);

  tileEl.addEventListener("dragstart", (event) => {
    tileDragState.dragIndex = index;
    tileEl.classList.add("is-dragging");
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", String(index));
    }
  });

  tileEl.addEventListener("dragend", () => {
    tileDragState.dragIndex = null;
    tileEl.classList.remove("is-dragging");
    document.querySelectorAll(".tile.drop-target").forEach((node) => node.classList.remove("drop-target"));
  });

  tileEl.addEventListener("dragover", (event) => {
    event.preventDefault();
    if (tileDragState.dragIndex === null || tileDragState.dragIndex === index) return;
    tileEl.classList.add("drop-target");
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "move";
    }
  });

  tileEl.addEventListener("dragleave", () => {
    tileEl.classList.remove("drop-target");
  });

  tileEl.addEventListener("drop", async (event) => {
    event.preventDefault();
    tileEl.classList.remove("drop-target");
    const fromIndex = tileDragState.dragIndex ?? Number(event.dataTransfer?.getData("text/plain"));
    if (!Number.isInteger(fromIndex)) return;
    moveTile(fromIndex, index);
    renderTiles();
    try {
      await persistConfig();
      renderTiles();
    } catch (error) {
      console.error("Failed to persist tile order", error);
    }
  });
};

const refreshTileMetadata = async (index = null) => {
  try {
    const response = await fetch("/api/tiles/metadata", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(index === null ? {} : { index }),
    });
    if (!response.ok) {
      throw new Error(`Metadata refresh failed with status ${response.status}`);
    }
    const payload = await response.json();
    if (payload?.config?.tiles) {
      state.tiles = payload.config.tiles;
      renderTiles();
    }
    if (Array.isArray(payload?.failures) && payload.failures.length) {
      console.warn("Some metadata fetches failed", payload.failures);
    }
  } catch (error) {
    console.error("Failed to refresh tile metadata", error);
  }
};

const renderTiles = () => {
  if (tileGroupsContainer) {
    tileGroupsContainer.innerHTML = "";
  }
  if (tilesContainer) {
    tilesContainer.innerHTML = "";
  }

  const groups = new Map();
  state.tiles.forEach((tile, index) => {
    const label = getTileGroupLabel(tile);
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label).push({ tile, index });
  });

  const sortedGroups = Array.from(groups.entries()).sort(([a], [b]) => a.localeCompare(b));

  sortedGroups.forEach(([groupLabel, entries]) => {
    const section = document.createElement("section");
    section.className = "tile-group";

    const heading = document.createElement("h3");
    heading.className = "tile-group-title";
    heading.textContent = groupLabel;
    section.appendChild(heading);

    const groupGrid = document.createElement("div");
    groupGrid.className = "tiles-grid";

    entries.forEach(({ tile, index }) => {
      const tileEl = document.createElement("article");
      tileEl.className = "tile";
      const refreshButton = document.createElement("button");
      refreshButton.type = "button";
      refreshButton.className = "tile-refresh";
      refreshButton.textContent = "↻";
      refreshButton.title = "Refresh metadata";
      refreshButton.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        refreshTileMetadata(index);
      });
      tileEl.appendChild(refreshButton);

      if (tile.pinned) {
        tileEl.classList.add("tile-pinned");
      }

      const img = document.createElement("img");
      img.src = thumbnailUrl(tile);
      img.alt = `${tile.title} thumbnail`;
      img.dataset.tileIndex = String(index);

      const label = document.createElement("div");
      label.className = "tile-title";
      const titleText = tileDisplayTitle(tile);
      label.textContent = titleText;

      const link = document.createElement("a");
      link.href = tile.url;
      link.target = "_blank";
      link.rel = "noreferrer";
      link.ariaLabel = `Open ${titleText}`;

      tileEl.appendChild(img);

      const meta = document.createElement("div");
      meta.className = "tile-meta";
      if (tile.favicon) {
        const icon = document.createElement("img");
        icon.className = "tile-favicon";
        icon.src = tile.favicon;
        icon.alt = "";
        icon.loading = "lazy";
        icon.referrerPolicy = "no-referrer";
        icon.addEventListener("error", () => {
          icon.style.display = "none";
        });
        meta.appendChild(icon);
      }
      meta.appendChild(label);
      tileEl.appendChild(meta);
      if (tile.pinned) {
        const pin = document.createElement("span");
        pin.className = "tile-pin";
        pin.textContent = "Pinned";
        tileEl.appendChild(pin);
      }
      tileEl.appendChild(link);
      makeTileDraggable(tileEl, index);
      groupGrid.appendChild(tileEl);
    });

    section.appendChild(groupGrid);
    if (tileGroupsContainer) {
      tileGroupsContainer.appendChild(section);
    } else if (tilesContainer) {
      tilesContainer.appendChild(groupGrid);
    }
  });
};

const refreshTilePreviews = () => {
  const timestamp = Date.now();
  const tileRoot = tileGroupsContainer || tilesContainer;
  if (!tileRoot) return;
  tileRoot.querySelectorAll("img").forEach((img) => {
    const index = Number(img.dataset.tileIndex);
    const tile = state.tiles[index];
    if (!tile) return;
    const baseUrl = thumbnailUrl(tile);
    try {
      const url = new URL(baseUrl, window.location.origin);
      url.searchParams.set("ts", String(timestamp));
      img.src = url.toString();
    } catch (error) {
      console.warn("Failed to refresh tile preview", error);
    }
  });
};

const renderStatusList = (container, items, keyLabel, historyMap) => {
  container.innerHTML = "";
  items.forEach((item) => {
    const wrapper = document.createElement("div");
    wrapper.className = "status-item";

    const info = document.createElement("div");
    info.className = "status-info";
    const title = document.createElement("strong");
    title.textContent = item.name || item.id || "Unnamed";
    const sub = document.createElement("span");
    sub.textContent = item[keyLabel] ?? item.url ?? "";
    info.appendChild(title);
    info.appendChild(document.createElement("br"));
    info.appendChild(sub);

    const meta = document.createElement("div");
    meta.className = "status-meta-row";
    const sparkline = document.createElement("div");
    sparkline.className = "status-sparkline";
    const historyKey = item.id || item.name;
    const history = historyKey ? historyMap?.get(historyKey) ?? [] : [];
    if (history.length) {
      history.forEach((online) => {
        const bar = document.createElement("span");
        bar.className = online ? "spark online" : "spark offline";
        sparkline.appendChild(bar);
      });
    } else {
      const empty = document.createElement("span");
      empty.className = "spark empty";
      sparkline.appendChild(empty);
    }
    const badge = document.createElement("div");
    badge.className = "uptime-badge";
    if (history.length) {
      const onlineCount = history.filter(Boolean).length;
      const uptime = Math.round((onlineCount / history.length) * 100);
      badge.textContent = `Uptime ${uptime}%`;
    } else {
      badge.textContent = "Uptime --";
    }
    meta.appendChild(badge);
    meta.appendChild(sparkline);

    const indicator = document.createElement("div");
    indicator.className = "status-indicator";
    if (item.online) {
      indicator.classList.add("online");
    }

    wrapper.appendChild(info);
    wrapper.appendChild(meta);
    wrapper.appendChild(indicator);
    container.appendChild(wrapper);
  });
};

const renderAll = () => {
  renderTiles();
  renderStatusList(devicesContainer, state.devices, "address", state.history.devices);
  renderStatusList(servicesContainer, state.services, "check_url", state.history.services);
};

const formatLastUpdated = (timestamp) => {
  if (!timestamp) return "Last updated: pending";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "Last updated: unknown";
  return `Last updated: ${date.toLocaleTimeString()}`;
};

const renderStatusMeta = ({ last_checked: lastChecked, stale }) => {
  const label = `${formatLastUpdated(lastChecked)}${stale ? " (stale)" : ""}`;
  devicesUpdated.textContent = label;
  servicesUpdated.textContent = label;
  devicesUpdated.classList.toggle("is-stale", Boolean(stale));
  servicesUpdated.classList.toggle("is-stale", Boolean(stale));
  devicesContainer.classList.toggle("status-stale", Boolean(stale));
  servicesContainer.classList.toggle("status-stale", Boolean(stale));
};

const stringifyRuleMap = (value) => {
  if (!value || typeof value !== "object" || Array.isArray(value) || !Object.keys(value).length) return "";
  return JSON.stringify(value);
};

const formatServiceLine = (service) => {
  const extras = [
    service.method ? service.method.toUpperCase() : "",
    service.timeout ?? "",
    service.expected_status ?? "",
    service.path ?? "",
    service.contains_text ?? "",
    stringifyRuleMap(service.header_equals),
    stringifyRuleMap(service.json_path_equals),
  ].map((value) =>
    value === null || value === undefined ? "" : String(value).trim()
  );
  const parts = [service.name ?? "", service.url ?? "", ...extras].map((value) => String(value).trim());
  while (parts.length > 2 && !parts[parts.length - 1]) {
    parts.pop();
  }
  return parts.join(" | ");
};

const clearEditorErrors = () => {
  editorErrors.innerHTML = "";
  editorErrors.classList.remove("active");
  editorText.classList.remove("has-errors");
};

const showEditorErrors = (errors) => {
  editorErrors.innerHTML = "";
  if (!errors.length) {
    clearEditorErrors();
    return;
  }
  errors.forEach((message) => {
    const line = document.createElement("div");
    line.className = "editor-error-item";
    line.textContent = message;
    editorErrors.appendChild(line);
  });
  editorErrors.classList.add("active");
  editorText.classList.add("has-errors");
};

const openEditor = (target) => {
  currentEdit = target;
  editorTitle.textContent = `Edit ${target}`;
  clearEditorErrors();
  clearEditorStatus();
  if (target === "alerts") {
    const lines = [`debounce_seconds | ${state.alerts.debounce_seconds ?? 300}`];
    ALERT_CHANNELS.forEach((channel) => {
      const cfg = state.alerts[channel] || { enabled: false, target: "" };
      lines.push(`${channel} | ${cfg.enabled ? "on" : "off"} | ${cfg.target || ""}`);
    });
    editorText.value = lines.join("\n");
  } else {
    const items = state[target];
    editorText.value = items
      .map((item) => {
        if (target === "services") return formatServiceLine(item);
        const key = target === "devices" ? "address" : "url";
        if (target === "tiles") {
          return `${item.title} | ${item[key]} | ${item.group || ""} | ${item.pinned ? "true" : "false"}`;
        }
        return `${item.name} | ${item[key]}`;
      })
      .join("\n");
  }
  editor.classList.add("active");
  editor.setAttribute("aria-hidden", "false");
};

const closeEditor = () => {
  editor.classList.remove("active");
  editor.setAttribute("aria-hidden", "true");
  currentEdit = null;
  clearEditorErrors();
  clearEditorStatus();
};

const isValidUrl = (value) => {
  try {
    const url = new URL(value);
    return ["http:", "https:"].includes(url.protocol);
  } catch {
    return false;
  }
};

const isValidIpv4 = (value) => {
  const parts = value.split(".");
  if (parts.length !== 4) return false;
  return parts.every((part) => {
    if (!/^\d+$/.test(part)) return false;
    const num = Number(part);
    return num >= 0 && num <= 255;
  });
};

const isValidHostname = (value) => {
  if (!value) return false;
  if (value.toLowerCase() === "localhost") return true;
  if (value.length > 253) return false;
  const labels = value.split(".");
  return labels.every((label) => {
    if (!label || label.length > 63) return false;
    if (!/^[a-z0-9-]+$/i.test(label)) return false;
    return !label.startsWith("-") && !label.endsWith("-");
  });
};

const validateLines = (target, lines) => {
  const errors = [];
  const parsed = [];
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const lineNumber = index + 1;
    const parts = trimmed.split("|").map((part) => part.trim());

    if (target === "alerts") {
      const key = (parts[0] || "").toLowerCase();
      if (key === "debounce_seconds") {
        const value = Number(parts[1] || "");
        if (!Number.isInteger(value) || value < 0) {
          errors.push(`Line ${lineNumber}: debounce_seconds must be a non-negative integer.`);
          return;
        }
        parsed.push({ type: "debounce", value });
        return;
      }
      if (!ALERT_CHANNELS.includes(key)) {
        errors.push(`Line ${lineNumber}: unknown alert key '${key}'.`);
        return;
      }
      const enabledRaw = (parts[1] || "").toLowerCase();
      if (!["on", "off", "true", "false", "1", "0"].includes(enabledRaw)) {
        errors.push(`Line ${lineNumber}: enabled must be on/off.`);
      }
      const enabled = ["on", "true", "1"].includes(enabledRaw);
      const targetValue = parts.slice(2).join("|").trim();
      parsed.push({ type: "channel", channel: key, enabled, target: targetValue });
      return;
    }

    const label = target === "tiles" ? "title" : "name";
    const name = parts[0] || "";
    if (!name) {
      errors.push(`Line ${lineNumber}: ${label} is required.`);
    }

    if (target === "services") {
      const url = parts[1] || "";
      if (!url) {
        errors.push(`Line ${lineNumber}: URL is required.`);
        return;
      }
      if (!isValidUrl(url)) {
        errors.push(`Line ${lineNumber}: URL must be a valid http(s) address.`);
      }
      const rawMethod = parts[2] || "";
      const method = rawMethod.toUpperCase();
      if (rawMethod && !VALID_METHODS.has(method)) {
        errors.push(`Line ${lineNumber}: HTTP method must be one of ${Array.from(VALID_METHODS).join(", ")}.`);
      }
      const timeoutRaw = parts[3] || "";
      const timeout = timeoutRaw ? Number(timeoutRaw) : null;
      if (timeoutRaw && (!Number.isFinite(timeout) || timeout <= 0)) {
        errors.push(`Line ${lineNumber}: timeout must be a positive number.`);
      }
      const statusRaw = parts[4] || "";
      const expectedStatus = statusRaw ? Number(statusRaw) : null;
      if (statusRaw && (!Number.isInteger(expectedStatus) || expectedStatus < 100 || expectedStatus > 599)) {
        errors.push(`Line ${lineNumber}: expected status must be a valid HTTP code.`);
      }
      const path = parts[5] || "";
      const containsText = parts[6] || "";

      let headerEquals = null;
      const headerRaw = parts[7] || "";
      if (headerRaw) {
        try {
          headerEquals = JSON.parse(headerRaw);
        } catch {
          errors.push(`Line ${lineNumber}: header_equals must be valid JSON.`);
        }
        if (headerEquals !== null && (typeof headerEquals !== "object" || Array.isArray(headerEquals))) {
          errors.push(`Line ${lineNumber}: header_equals must be a JSON object.`);
        } else if (headerEquals !== null) {
          const invalidEntry = Object.entries(headerEquals).find(
            ([key, value]) => !key.trim() || typeof value !== "string"
          );
          if (invalidEntry) {
            errors.push(`Line ${lineNumber}: header_equals values must be strings.`);
          }
        }
      }

      let jsonPathEquals = null;
      const jsonPathRaw = parts.slice(8).join("|").trim();
      if (jsonPathRaw) {
        try {
          jsonPathEquals = JSON.parse(jsonPathRaw);
        } catch {
          errors.push(`Line ${lineNumber}: json_path_equals must be valid JSON.`);
        }
        if (jsonPathEquals !== null && (typeof jsonPathEquals !== "object" || Array.isArray(jsonPathEquals))) {
          errors.push(`Line ${lineNumber}: json_path_equals must be a JSON object.`);
        }
      }

      parsed.push({ name, url, method, timeout, expectedStatus, path, containsText, headerEquals, jsonPathEquals });
      return;
    }

    const right = parts.slice(1).join("|").trim();
    if (!right) {
      errors.push(`Line ${lineNumber}: ${target === "devices" ? "address" : "URL"} is required.`);
      return;
    }
    if (target === "devices") {
      if (!(isValidIpv4(right) || isValidHostname(right))) {
        errors.push(`Line ${lineNumber}: address must be a valid IP or hostname.`);
      }
    } else if (!isValidUrl(right)) {
      errors.push(`Line ${lineNumber}: URL must be a valid http(s) address.`);
    }
    if (target === "tiles") {
      const extras = parts.slice(2);
      const group = (extras[0] || "").trim();
      const pinnedRaw = (extras[1] || "").trim().toLowerCase();
      const allowedPinnedValues = ["", "true", "false", "1", "0", "yes", "no", "on", "off"];
      if (!allowedPinnedValues.includes(pinnedRaw)) {
        errors.push(`Line ${lineNumber}: pinned must be true/false.`);
      }
      const pinned = ["true", "1", "yes", "on"].includes(pinnedRaw);
      parsed.push({ left: name, right, group, pinned });
      return;
    }
    parsed.push({ left: name, right });
  });
  return { errors, parsed };
};

const saveEditor = async () => {
  if (!currentEdit) return;
  const lines = editorText.value.split("\n");
  const { errors, parsed } = validateLines(currentEdit, lines);
  if (errors.length) {
    showEditorErrors(errors);
    clearEditorStatus();
    return;
  }
  clearEditorErrors();
  showEditorStatus("Saving…", "info");

  if (currentEdit === "tiles") {
    state.tiles = parsed.map((item) => ({
      title: item.left,
      url: item.right,
      ...(item.group ? { group: item.group } : {}),
      ...(item.pinned ? { pinned: true } : {}),
    }));
  }

  if (currentEdit === "devices") {
    state.devices = parsed.map((item, index) => ({
      id: state.devices[index]?.id,
      name: item.left,
      address: item.right,
      online: false,
    }));
  }

  if (currentEdit === "services") {
    state.services = parsed.map((item, index) => ({
      id: state.services[index]?.id,
      name: item.name,
      url: item.url,
      method: item.method || "GET",
      timeout: item.timeout ?? 2,
      expected_status: item.expectedStatus ?? 200,
      path: item.path || "",
      contains_text: item.containsText || "",
      ...(item.headerEquals && Object.keys(item.headerEquals).length ? { header_equals: item.headerEquals } : {}),
      ...(item.jsonPathEquals && Object.keys(item.jsonPathEquals).length ? { json_path_equals: item.jsonPathEquals } : {}),
      online: false,
    }));
  }

  if (currentEdit === "alerts") {
    const nextAlerts = {
      debounce_seconds: state.alerts.debounce_seconds ?? 300,
      email: { ...(state.alerts.email || { enabled: false, target: "" }) },
      webhook: { ...(state.alerts.webhook || { enabled: false, target: "" }) },
      ntfy: { ...(state.alerts.ntfy || { enabled: false, target: "" }) },
      slack: { ...(state.alerts.slack || { enabled: false, target: "" }) },
      discord: { ...(state.alerts.discord || { enabled: false, target: "" }) },
    };
    parsed.forEach((item) => {
      if (item.type === "debounce") nextAlerts.debounce_seconds = item.value;
      if (item.type === "channel") nextAlerts[item.channel] = { enabled: item.enabled, target: item.target || "" };
    });
    state.alerts = nextAlerts;
  }

  try {
    await persistConfig();
  } catch (error) {
    console.error("Failed to save config", error);
    showEditorStatus("Could not save changes. Please try again.", "error");
    return;
  }

  renderAll();
  showEditorStatus("Saved", "success", { autoClearMs: 1200 });
  setTimeout(() => {
    closeEditor();
  }, 500);
};

const buildHistoryMap = (entries, key) => {
  const map = new Map();
  entries.forEach((entry) => {
    (entry[key] || []).forEach((item) => {
      const historyKey = item.id || item.name;
      if (!historyKey) return;
      if (!map.has(historyKey)) {
        map.set(historyKey, []);
      }
      map.get(historyKey).push(Boolean(item.online));
    });
  });
  return map;
};

const refreshStatuses = async () => {
  try {
    const [statusResponse, historyResponse] = await Promise.all([
      fetch("/api/status"),
      fetch("/api/status/history?limit=60"),
    ]);
    if (!statusResponse.ok) return;
    const payload = await statusResponse.json();
    let historyPayload = null;
    if (historyResponse.ok) {
      historyPayload = await historyResponse.json();
    }
    state.devices = payload.devices;
    state.services = payload.services;
    if (historyPayload?.entries) {
      state.history.devices = buildHistoryMap(historyPayload.entries, "devices");
      state.history.services = buildHistoryMap(historyPayload.entries, "services");
    }
    renderStatusMeta(payload);
    renderStatusList(devicesContainer, state.devices, "address", state.history.devices);
    renderStatusList(servicesContainer, state.services, "check_url", state.history.services);
  } catch (error) {
    console.error("Status refresh failed", error);
  }
};

renderAll();
refreshStatuses();
setInterval(refreshStatuses, 15000);

const refreshIntervalHours = Number(state.tileRefreshHours ?? 6);
if (Number.isFinite(refreshIntervalHours) && refreshIntervalHours > 0) {
  setInterval(refreshTilePreviews, refreshIntervalHours * 60 * 60 * 1000);
}

Array.from(document.querySelectorAll(".edit-button")).forEach((button) => {
  button.addEventListener("click", () => openEditor(button.dataset.edit));
});

editorClose.addEventListener("click", closeEditor);
editor.addEventListener("click", (event) => {
  if (event.target === editor) closeEditor();
});
editorSave.addEventListener("click", saveEditor);
editorText.addEventListener("input", clearEditorErrors);

const refreshMetadataButton = document.getElementById("refresh-metadata");
if (refreshMetadataButton) {
  refreshMetadataButton.addEventListener("click", () => {
    refreshTileMetadata();
  });
}
