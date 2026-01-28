from __future__ import annotations

import hashlib
import json
import subprocess
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional
from urllib.parse import urljoin

import requests
from flask import Flask, jsonify, render_template, request, send_from_directory

app = Flask(__name__)

BASE_DIR = Path(__file__).resolve().parent
CONFIG_DIR = BASE_DIR / "config"
CONFIG_PATH = CONFIG_DIR / "dashboard.json"
PREVIEW_DIR = BASE_DIR / "previews"
STATUS_REFRESH_SECONDS = 20
STATUS_STALE_SECONDS = 60

STATUS_LOCK = threading.Lock()
STATUS_CACHE: Dict[str, Any] = {
    "devices": [],
    "services": [],
    "last_checked": None,
}
STATUS_SCHEDULER_STARTED = False


DEFAULT_CONFIG: Dict[str, List[Dict[str, Any]]] = {
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
        {
            "name": "Jellyfin",
            "url": "https://demo.jellyfin.org/stable",
            "method": "GET",
            "timeout": 2,
            "expected_status": 200,
            "path": "",
        },
        {
            "name": "Kavita",
            "url": "https://www.kavitareader.com/",
            "method": "GET",
            "timeout": 2,
            "expected_status": 200,
            "path": "",
        },
        {
            "name": "Calibre",
            "url": "https://calibre-ebook.com/",
            "method": "GET",
            "timeout": 2,
            "expected_status": 200,
            "path": "",
        },
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


def _service_timeout(raw_timeout: Any) -> float:
    try:
        timeout = float(raw_timeout)
        return timeout if timeout > 0 else 2.0
    except (TypeError, ValueError):
        return 2.0


def _service_expected_status(raw_status: Any) -> int:
    try:
        status = int(raw_status)
        if 100 <= status <= 599:
            return status
    except (TypeError, ValueError):
        pass
    return 200


def build_service_check(service: Dict[str, Any]) -> Dict[str, Any]:
    method = str(service.get("method", "GET") or "GET").strip().upper() or "GET"
    timeout = _service_timeout(service.get("timeout", 2))
    expected_status = _service_expected_status(service.get("expected_status", 200))
    path = str(service.get("path", "") or "").strip()
    url = str(service.get("url", "") or "").strip()
    check_url = urljoin(f"{url.rstrip('/')}/", path.lstrip("/")) if path else url
    return {
        "method": method,
        "timeout": timeout,
        "expected_status": expected_status,
        "path": path,
        "check_url": check_url,
        "url": url,
    }


def check_service(service: Dict[str, Any]) -> Dict[str, Any]:
    settings = build_service_check(service)
    started = time.perf_counter()
    try:
        response = requests.request(settings["method"], settings["check_url"], timeout=settings["timeout"])
        elapsed_ms = (time.perf_counter() - started) * 1000
        online = response.status_code == settings["expected_status"]
        return {
            **settings,
            "status_code": response.status_code,
            "response_time_ms": round(elapsed_ms, 2),
            "online": online,
        }
    except requests.RequestException as exc:
        elapsed_ms = (time.perf_counter() - started) * 1000
        return {
            **settings,
            "status_code": None,
            "response_time_ms": round(elapsed_ms, 2),
            "online": False,
            "error": str(exc),
        }


def build_status_snapshot(config: Dict[str, Any]) -> Dict[str, Any]:
    device_statuses = [
        {"name": device["name"], "address": device["address"], "online": ping_device(device["address"])}
        for device in config["devices"]
    ]
    service_statuses = []
    for service in config["services"]:
        result = check_service(service)
        service_statuses.append(
            {
                "name": service.get("name", ""),
                "url": result["url"],
                "check_url": result["check_url"],
                "method": result["method"],
                "timeout": result["timeout"],
                "expected_status": result["expected_status"],
                "path": result["path"],
                "status_code": result["status_code"],
                "response_time_ms": result["response_time_ms"],
                "online": result["online"],
                "error": result.get("error"),
            }
        )
    return {"devices": device_statuses, "services": service_statuses}


def update_status_cache() -> None:
    config = load_config()
    snapshot = build_status_snapshot(config)
    with STATUS_LOCK:
        STATUS_CACHE["devices"] = snapshot["devices"]
        STATUS_CACHE["services"] = snapshot["services"]
        STATUS_CACHE["last_checked"] = time.time()


def status_worker() -> None:
    while True:
        update_status_cache()
        time.sleep(STATUS_REFRESH_SECONDS)


def start_status_scheduler() -> None:
    global STATUS_SCHEDULER_STARTED
    if STATUS_SCHEDULER_STARTED:
        return
    STATUS_SCHEDULER_STARTED = True
    thread = threading.Thread(target=status_worker, daemon=True)
    thread.start()


def status_payload() -> Dict[str, Any]:
    with STATUS_LOCK:
        devices = list(STATUS_CACHE["devices"])
        services = list(STATUS_CACHE["services"])
        last_checked = STATUS_CACHE["last_checked"]
    now = time.time()
    age_seconds: Optional[float] = None
    if last_checked is not None:
        age_seconds = now - last_checked
    stale = last_checked is None or age_seconds is None or age_seconds > STATUS_STALE_SECONDS
    last_checked_iso = (
        datetime.fromtimestamp(last_checked, tz=timezone.utc).isoformat()
        if last_checked is not None
        else None
    )
    return {
        "devices": devices,
        "services": services,
        "last_checked": last_checked_iso,
        "age_seconds": age_seconds,
        "stale": stale,
    }


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
    return jsonify(status_payload())


@app.before_request
def ensure_scheduler_started() -> None:
    start_status_scheduler()


if __name__ == "__main__":
    start_status_scheduler()
    app.run(host="0.0.0.0", port=5000, debug=True)
