from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any, Dict, List

import requests
from flask import Flask, jsonify, render_template, request, send_from_directory

app = Flask(__name__)

BASE_DIR = Path(__file__).resolve().parent
CONFIG_DIR = BASE_DIR / "config"
CONFIG_PATH = CONFIG_DIR / "dashboard.json"
PREVIEW_DIR = BASE_DIR / "previews"


DEFAULT_CONFIG: Dict[str, List[Dict[str, str]]] = {
    "tiles": [
        {"title": "Docs", "url": "https://docs.python.org/3/"},
        {"title": "Grafana", "url": "https://grafana.com/"},
        {"title": "Jellyfin", "url": "https://jellyfin.org/"},
        {"title": "Kavita", "url": "https://www.kavitareader.com/"},
    ],
    "devices": [
        {"name": "Router", "address": "192.168.1.1"},
        {"name": "NAS", "address": "192.168.1.10"},
        {"name": "Desktop", "address": "192.168.1.20"},
    ],
    "services": [
        {"name": "Jellyfin", "url": "https://demo.jellyfin.org/stable"},
        {"name": "Kavita", "url": "https://www.kavitareader.com/"},
        {"name": "Calibre", "url": "https://calibre-ebook.com/"},
    ],
}


def ensure_directories() -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)


def preview_filename(url: str) -> str:
    digest = hashlib.sha1(url.encode("utf-8")).hexdigest()
    return f"{digest[:12]}.jpg"


def fetch_preview(url: str, destination: Path) -> None:
    thumbnail_url = f"https://image.thum.io/get/width/800/{url}"
    response = requests.get(thumbnail_url, timeout=10)
    response.raise_for_status()
    destination.write_bytes(response.content)


def normalize_tiles(tiles: List[Dict[str, str]]) -> List[Dict[str, str]]:
    normalized = []
    for tile in tiles:
        title = tile.get("title", "").strip()
        url = tile.get("url", "").strip()
        if not title or not url:
            continue
        raw_preview = tile.get("preview") or ""
        filename = Path(raw_preview).name if raw_preview else preview_filename(url)
        preview_path = PREVIEW_DIR / filename
        if not preview_path.exists():
            try:
                fetch_preview(url, preview_path)
            except requests.RequestException:
                pass
        normalized.append({"title": title, "url": url, "preview": filename})
    return normalized


def load_config() -> Dict[str, Any]:
    ensure_directories()
    if CONFIG_PATH.exists():
        with CONFIG_PATH.open("r", encoding="utf-8") as handle:
            data = json.load(handle)
    else:
        data = DEFAULT_CONFIG
    data = {
        "tiles": normalize_tiles(data.get("tiles", [])),
        "devices": data.get("devices", []),
        "services": data.get("services", []),
    }
    save_config(data)
    return data


def save_config(config: Dict[str, Any]) -> None:
    ensure_directories()
    with CONFIG_PATH.open("w", encoding="utf-8") as handle:
        json.dump(config, handle, indent=2)


def serialize_config(config: Dict[str, Any]) -> Dict[str, Any]:
    tiles = [
        {
            **tile,
            "preview": f"/previews/{tile['preview']}" if tile.get("preview") else "",
        }
        for tile in config.get("tiles", [])
    ]
    return {
        "tiles": tiles,
        "devices": config.get("devices", []),
        "services": config.get("services", []),
    }


@app.route("/")
def index() -> str:
    config = serialize_config(load_config())
    tiles = config["tiles"]
    devices = config["devices"]
    services = config["services"]
    return render_template(
        "index.html",
        tiles_json=json.dumps(tiles),
        devices_json=json.dumps(devices),
        services_json=json.dumps(services),
    )


def ping_device(address: str) -> bool:
    try:
        result = subprocess.run(
            ["ping", "-c", "1", "-W", "1", address],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return False
    return result.returncode == 0


def check_service(url: str) -> bool:
    try:
        response = requests.get(url, timeout=2)
        return response.ok
    except requests.RequestException:
        return False


@app.route("/previews/<path:filename>")
def previews(filename: str):
    return send_from_directory(PREVIEW_DIR, filename)


@app.route("/api/config", methods=["GET", "POST"])
def config():
    if request.method == "GET":
        return jsonify(serialize_config(load_config()))

    payload = request.get_json(silent=True) or {}
    current = load_config()
    tiles = payload.get("tiles", current["tiles"])
    devices = payload.get("devices", current["devices"])
    services = payload.get("services", current["services"])
    updated = {
        "tiles": normalize_tiles(tiles),
        "devices": devices,
        "services": services,
    }
    save_config(updated)
    return jsonify(serialize_config(updated))


@app.route("/api/status")
def status():
    config = load_config()
    device_statuses = [
        {"name": device["name"], "address": device["address"], "online": ping_device(device["address"])}
        for device in config["devices"]
    ]
    service_statuses = [
        {"name": service["name"], "url": service["url"], "online": check_service(service["url"])}
        for service in config["services"]
    ]
    return jsonify({"devices": device_statuses, "services": service_statuses})


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, debug=True)
