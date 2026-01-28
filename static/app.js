const data = window.__DASHBOARD_DATA__;

const state = {
  tiles: [...data.tiles],
  devices: [...data.devices],
  services: [...data.services],
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
  state.tiles.forEach((tile) => {
    const tileEl = document.createElement("article");
    tileEl.className = "tile";

    const img = document.createElement("img");
    img.src = thumbnailUrl(tile);
    img.alt = `${tile.title} thumbnail`;

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

const renderStatusList = (container, items, keyLabel) => {
  container.innerHTML = "";
  items.forEach((item) => {
    const wrapper = document.createElement("div");
    wrapper.className = "status-item";

    const info = document.createElement("div");
    const title = document.createElement("strong");
    title.textContent = item.name;
    const sub = document.createElement("span");
    sub.textContent = item[keyLabel];
    info.appendChild(title);
    info.appendChild(document.createElement("br"));
    info.appendChild(sub);

    const indicator = document.createElement("div");
    indicator.className = "status-indicator";
    if (item.online) {
      indicator.classList.add("online");
    }

    wrapper.appendChild(info);
    wrapper.appendChild(indicator);
    container.appendChild(wrapper);
  });
};

const renderAll = () => {
  renderTiles();
  renderStatusList(devicesContainer, state.devices, "address");
  renderStatusList(servicesContainer, state.services, "url");
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

const openEditor = (target) => {
  currentEdit = target;
  editorTitle.textContent = `Edit ${target}`;
  clearEditorErrors();
  const items = state[target];
  editorText.value = items
    .map((item) => {
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

const validateLines = (target, lines) => {
  const errors = [];
  const parsed = [];
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const parts = trimmed.split("|");
    const left = (parts.shift() || "").trim();
    const right = parts.join("|").trim();
    const lineNumber = index + 1;
    const label = target === "tiles" ? "title" : "name";
    if (!left) {
      errors.push(`Line ${lineNumber}: ${label} is required.`);
    }
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
    parsed.push({ left, right });
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
    state.services = normalized.map((item) => ({
      name: item.left,
      url: item.right,
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

const refreshStatuses = async () => {
  try {
    const response = await fetch("/api/status");
    if (!response.ok) return;
    const payload = await response.json();
    state.devices = payload.devices;
    state.services = payload.services;
    renderStatusMeta(payload);
    renderStatusList(devicesContainer, state.devices, "address");
    renderStatusList(servicesContainer, state.services, "url");
  } catch (error) {
    console.error("Status refresh failed", error);
  }
};

renderAll();
refreshStatuses();
setInterval(refreshStatuses, 15000);

Array.from(document.querySelectorAll(".edit-button")).forEach((button) => {
  button.addEventListener("click", () => openEditor(button.dataset.edit));
});

editorClose.addEventListener("click", closeEditor);
editor.addEventListener("click", (event) => {
  if (event.target === editor) closeEditor();
});
editorSave.addEventListener("click", saveEditor);
editorText.addEventListener("input", clearEditorErrors);
