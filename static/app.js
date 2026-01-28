const data = window.__DASHBOARD_DATA__;

const state = {
  tiles: [...data.tiles],
  devices: [...data.devices],
  services: [...data.services],
  tileRefreshHours: data.tile_refresh_hours,
  history: {
    devices: new Map(),
    services: new Map(),
  },
};

const tilesContainer = document.getElementById("tiles");
const devicesContainer = document.getElementById("devices");
const servicesContainer = document.getElementById("services");
const devicesUpdated = document.getElementById("devices-updated");
const servicesUpdated = document.getElementById("services-updated");

const editor = document.getElementById("editor");
const editorTitle = document.getElementById("editor-title");
const editorText = document.getElementById("editor-text");
const editorSave = document.getElementById("editor-save");
const editorClose = document.getElementById("editor-close");
const editorErrors = document.createElement("div");
editorErrors.id = "editor-errors";
editorErrors.className = "editor-errors";
editorErrors.setAttribute("role", "alert");
editorErrors.setAttribute("aria-live", "polite");
editorText.insertAdjacentElement("afterend", editorErrors);

let currentEdit = null;

const thumbnailUrl = (tile) =>
  tile.preview || `https://image.thum.io/get/width/800/${encodeURIComponent(tile.url)}`;

const renderTiles = () => {
  tilesContainer.innerHTML = "";
  state.tiles.forEach((tile, index) => {
    const tileEl = document.createElement("article");
    tileEl.className = "tile";

    const img = document.createElement("img");
    img.src = thumbnailUrl(tile);
    img.alt = `${tile.title} thumbnail`;
    img.dataset.tileIndex = String(index);

    const label = document.createElement("div");
    label.className = "tile-title";
    label.textContent = tile.title;

    const link = document.createElement("a");
    link.href = tile.url;
    link.target = "_blank";
    link.rel = "noreferrer";
    link.ariaLabel = `Open ${tile.title}`;

    tileEl.appendChild(img);
    tileEl.appendChild(label);
    tileEl.appendChild(link);
    tilesContainer.appendChild(tileEl);
  });
};

const refreshTilePreviews = () => {
  const timestamp = Date.now();
  tilesContainer.querySelectorAll("img").forEach((img) => {
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
    title.textContent = item.name;
    const sub = document.createElement("span");
    sub.textContent = item[keyLabel] ?? item.url ?? "";
    info.appendChild(title);
    info.appendChild(document.createElement("br"));
    info.appendChild(sub);

    const meta = document.createElement("div");
    meta.className = "status-meta-row";
    const sparkline = document.createElement("div");
    sparkline.className = "status-sparkline";
    const history = historyMap?.get(item.name) ?? [];
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

const formatServiceLine = (service) => {
  const extras = [
    service.method ? service.method.toUpperCase() : "",
    service.timeout ?? "",
    service.expected_status ?? "",
    service.path ?? "",
  ].map((value) => (value === null || value === undefined ? "" : String(value).trim()));
  const parts = [service.name ?? "", service.url ?? "", ...extras].map((value) => String(value).trim());
  while (parts.length > 2 && !parts[parts.length - 1]) {
    parts.pop();
  }
  return parts.join(" | ");
};

const openEditor = (target) => {
  currentEdit = target;
  editorTitle.textContent = `Edit ${target}`;
  clearEditorErrors();
  const items = state[target];
  editorText.value = items
    .map((item) => {
      if (target === "services") {
        return formatServiceLine(item);
      }
      const key = target === "devices" ? "address" : "url";
      return `${item.name} | ${item[key]}`;
    })
    .join("\n");
  editor.classList.add("active");
  editor.setAttribute("aria-hidden", "false");
};

const closeEditor = () => {
  editor.classList.remove("active");
  editor.setAttribute("aria-hidden", "true");
  currentEdit = null;
  clearEditorErrors();
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

const VALID_METHODS = new Set(["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]);

const validateLines = (target, lines) => {
  const errors = [];
  const parsed = [];
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const lineNumber = index + 1;
    const parts = trimmed.split("|").map((part) => part.trim());
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
      const path = parts.slice(5).join("|").trim();
      parsed.push({
        name,
        url,
        method,
        timeout,
        expectedStatus,
        path,
      });
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
    return;
  }
  clearEditorErrors();
  const normalized = parsed.map((item) => ({
    left: item.left,
    right: item.right,
  }));

  if (currentEdit === "tiles") {
    state.tiles = normalized.map((item) => ({
      title: item.left,
      url: item.right,
    }));
  }

  if (currentEdit === "devices") {
    state.devices = normalized.map((item) => ({
      name: item.left,
      address: item.right,
      online: false,
    }));
  }

  if (currentEdit === "services") {
    state.services = parsed.map((item) => ({
      name: item.name,
      url: item.url,
      method: item.method || "GET",
      timeout: item.timeout ?? 2,
      expected_status: item.expectedStatus ?? 200,
      path: item.path || "",
      online: false,
    }));
  }

  try {
    const response = await fetch("/api/config", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        tiles: state.tiles,
        devices: state.devices,
        services: state.services,
      }),
    });
    if (response.ok) {
      const updated = await response.json();
      state.tiles = updated.tiles;
      state.devices = updated.devices;
      state.services = updated.services;
    }
  } catch (error) {
    console.error("Failed to save config", error);
  }

  renderAll();
  closeEditor();
};

const buildHistoryMap = (entries, key) => {
  const map = new Map();
  entries.forEach((entry) => {
    (entry[key] || []).forEach((item) => {
      if (!item.name) return;
      if (!map.has(item.name)) {
        map.set(item.name, []);
      }
      map.get(item.name).push(Boolean(item.online));
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
