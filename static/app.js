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
};

const saveEditor = async () => {
  if (!currentEdit) return;
  const lines = editorText.value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);

  if (currentEdit === "tiles") {
    state.tiles = lines.map((line) => {
      const [title, url] = line.split("|").map((part) => part.trim());
      return { title, url };
    });
  }

  if (currentEdit === "devices") {
    state.devices = lines.map((line) => {
      const [name, address] = line.split("|").map((part) => part.trim());
      return { name, address, online: false };
    });
  }

  if (currentEdit === "services") {
    state.services = lines.map((line) => {
      const [name, url] = line.split("|").map((part) => part.trim());
      return { name, url, online: false };
    });
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
