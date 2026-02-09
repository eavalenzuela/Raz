from __future__ import annotations

import hashlib
import ipaddress
import json
import queue
import re
import subprocess
import threading
import time
import uuid
from copy import deepcopy
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional
from urllib.parse import urljoin, urlparse

import requests
from flask import Flask, jsonify, render_template, request, send_from_directory

app = Flask(__name__)

BASE_DIR = Path(__file__).resolve().parent
CONFIG_DIR = BASE_DIR / "config"
CONFIG_PATH = CONFIG_DIR / "dashboard.json"
PREVIEW_DIR = BASE_DIR / "previews"
STATUS_HISTORY_PATH = CONFIG_DIR / "status_history.jsonl"
STATUS_REFRESH_SECONDS = 20
STATUS_STALE_SECONDS = 60
STATUS_HISTORY_LIMIT = 240
DEFAULT_TILE_REFRESH_HOURS = 6
DEFAULT_ALERT_DEBOUNCE_SECONDS = 300
ALLOWED_SERVICE_METHODS = {"GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"}
ALERT_CHANNELS = ("email", "webhook", "ntfy", "slack", "discord")

STATUS_LOCK = threading.Lock()
STATUS_CACHE: Dict[str, Any] = {
    "devices": [],
    "services": [],
    "last_checked": None,
}
STATUS_SCHEDULER_STARTED = False
PREVIEW_FETCH_QUEUE: "queue.Queue[tuple[str, str]]" = queue.Queue()
PREVIEW_PENDING: set[str] = set()
PREVIEW_LOCK = threading.Lock()
PREVIEW_WORKER_STARTED = False
PLACEHOLDER_PREVIEW = "preview-placeholder.svg"
ALERT_LAST_EMITTED: Dict[str, float] = {}


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
    "tile_refresh_hours": DEFAULT_TILE_REFRESH_HOURS,
    "alerts": {
        "debounce_seconds": DEFAULT_ALERT_DEBOUNCE_SECONDS,
        "email": {"enabled": False, "target": ""},
        "webhook": {"enabled": False, "target": ""},
        "ntfy": {"enabled": False, "target": ""},
        "slack": {"enabled": False, "target": ""},
        "discord": {"enabled": False, "target": ""},
    },
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


def enqueue_preview_fetch(url: str, filename: str) -> None:
    preview_path = PREVIEW_DIR / filename
    if preview_path.exists():
        return
    with PREVIEW_LOCK:
        if filename in PREVIEW_PENDING:
            return
        PREVIEW_PENDING.add(filename)
    PREVIEW_FETCH_QUEUE.put((url, filename))


def preview_worker() -> None:
    while True:
        url, filename = PREVIEW_FETCH_QUEUE.get()
        try:
            preview_path = PREVIEW_DIR / filename
            if not preview_path.exists():
                try:
                    fetch_preview(url, preview_path)
                except requests.RequestException:
                    pass
        finally:
            with PREVIEW_LOCK:
                PREVIEW_PENDING.discard(filename)
            PREVIEW_FETCH_QUEUE.task_done()


def start_preview_worker() -> None:
    global PREVIEW_WORKER_STARTED
    if PREVIEW_WORKER_STARTED:
        return
    PREVIEW_WORKER_STARTED = True
    thread = threading.Thread(target=preview_worker, daemon=True)
    thread.start()


def normalize_tiles(tiles: List[Dict[str, str]]) -> List[Dict[str, str]]:
    normalized = []
    for tile in tiles:
        title = tile.get("title", "").strip()
        url = tile.get("url", "").strip()
        if not title or not url:
            continue
        raw_preview = tile.get("preview") or ""
        filename = Path(raw_preview).name if raw_preview else preview_filename(url)
        enqueue_preview_fetch(url, filename)
        normalized.append({"title": title, "url": url, "preview": filename})
    return normalized


def _is_valid_url(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    parsed = urlparse(value.strip())
    return parsed.scheme in {"http", "https"} and bool(parsed.netloc)


def _is_valid_host_or_ip(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    host = value.strip()
    if not host:
        return False
    try:
        ipaddress.ip_address(host)
        return True
    except ValueError:
        pass
    hostname_pattern = re.compile(
        r"^(?=.{1,253}$)(?:(?!-)[A-Za-z0-9-]{1,63}(?<!-)\\.)*(?!-)[A-Za-z0-9-]{1,63}(?<!-)$"
    )
    return bool(hostname_pattern.match(host))


def _validate_tiles(raw_tiles: Any) -> tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    if not isinstance(raw_tiles, list):
        return [], [{"section": "tiles", "message": "must be a list"}]
    validated: List[Dict[str, Any]] = []
    errors: List[Dict[str, Any]] = []
    for idx, tile in enumerate(raw_tiles):
        if not isinstance(tile, dict):
            errors.append({"section": "tiles", "index": idx, "message": "must be an object"})
            continue
        title = str(tile.get("title", "") or "").strip()
        url = str(tile.get("url", "") or "").strip()
        if not title:
            errors.append({"section": "tiles", "index": idx, "field": "title", "message": "must be non-empty"})
        if not _is_valid_url(url):
            errors.append(
                {"section": "tiles", "index": idx, "field": "url", "message": "must be a valid http/https URL"}
            )
        if title and _is_valid_url(url):
            validated.append({"title": title, "url": url, "preview": tile.get("preview", "")})
    return validated, errors


def _validate_devices(raw_devices: Any) -> tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    if not isinstance(raw_devices, list):
        return [], [{"section": "devices", "message": "must be a list"}]
    validated: List[Dict[str, Any]] = []
    errors: List[Dict[str, Any]] = []
    for idx, device in enumerate(raw_devices):
        if not isinstance(device, dict):
            errors.append({"section": "devices", "index": idx, "message": "must be an object"})
            continue
        name = str(device.get("name", "") or "").strip()
        address = str(device.get("address", "") or "").strip()
        device_id = str(device.get("id", "") or "").strip()
        if not name:
            errors.append({"section": "devices", "index": idx, "field": "name", "message": "must be non-empty"})
        if not _is_valid_host_or_ip(address):
            errors.append(
                {"section": "devices", "index": idx, "field": "address", "message": "must be a valid host or IP"}
            )
        if name and _is_valid_host_or_ip(address):
            normalized = {"name": name, "address": address}
            if device_id:
                normalized["id"] = device_id
            validated.append(normalized)
    return validated, errors


def _validate_services(raw_services: Any) -> tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    if not isinstance(raw_services, list):
        return [], [{"section": "services", "message": "must be a list"}]
    validated: List[Dict[str, Any]] = []
    errors: List[Dict[str, Any]] = []
    for idx, service in enumerate(raw_services):
        if not isinstance(service, dict):
            errors.append({"section": "services", "index": idx, "message": "must be an object"})
            continue
        name = str(service.get("name", "") or "").strip()
        url = str(service.get("url", "") or "").strip()
        service_id = str(service.get("id", "") or "").strip()
        method = str(service.get("method", "GET") or "GET").strip().upper() or "GET"
        timeout, timeout_valid = _parse_service_timeout(service.get("timeout"))
        expected_status, expected_status_valid = _parse_service_expected_status(service.get("expected_status"))
        path_raw = service.get("path", "")
        path = str(path_raw).strip() if isinstance(path_raw, str) else None

        if not name:
            errors.append({"section": "services", "index": idx, "field": "name", "message": "must be non-empty"})
        if not _is_valid_url(url):
            errors.append(
                {
                    "section": "services",
                    "index": idx,
                    "field": "url",
                    "message": "must be a valid http/https URL",
                }
            )
        if method not in ALLOWED_SERVICE_METHODS:
            errors.append(
                {
                    "section": "services",
                    "index": idx,
                    "field": "method",
                    "message": f"must be one of {sorted(ALLOWED_SERVICE_METHODS)}",
                }
            )
        if not timeout_valid:
            errors.append({"section": "services", "index": idx, "field": "timeout", "message": "must be > 0"})
        if not expected_status_valid:
            errors.append(
                {
                    "section": "services",
                    "index": idx,
                    "field": "expected_status",
                    "message": "must be between 100 and 599",
                }
            )
        if path is None:
            errors.append({"section": "services", "index": idx, "field": "path", "message": "must be a string"})

        if (
            name
            and _is_valid_url(url)
            and method in ALLOWED_SERVICE_METHODS
            and timeout_valid
            and expected_status_valid
            and path is not None
        ):
            validated.append(
                {
                    "name": name,
                    "url": url,
                    "id": service_id,
                    "method": method,
                    "timeout": timeout,
                    "expected_status": expected_status,
                    "path": path,
                }
            )
            if not service_id:
                validated[-1].pop("id")
    return validated, errors


def _default_alerts() -> Dict[str, Any]:
    return deepcopy(DEFAULT_CONFIG["alerts"])


def normalize_alerts(raw_alerts: Any) -> Dict[str, Any]:
    alerts = _default_alerts()
    if not isinstance(raw_alerts, dict):
        return alerts
    try:
        debounce_seconds = int(raw_alerts.get("debounce_seconds", DEFAULT_ALERT_DEBOUNCE_SECONDS))
        alerts["debounce_seconds"] = debounce_seconds if debounce_seconds >= 0 else DEFAULT_ALERT_DEBOUNCE_SECONDS
    except (TypeError, ValueError):
        alerts["debounce_seconds"] = DEFAULT_ALERT_DEBOUNCE_SECONDS

    for channel in ALERT_CHANNELS:
        channel_raw = raw_alerts.get(channel, {})
        if isinstance(channel_raw, dict):
            alerts[channel] = {
                "enabled": bool(channel_raw.get("enabled", False)),
                "target": str(channel_raw.get("target", "") or "").strip(),
            }
    return alerts


def _validate_alerts(raw_alerts: Any) -> tuple[Dict[str, Any], List[Dict[str, Any]]]:
    if raw_alerts is None:
        return _default_alerts(), []
    if not isinstance(raw_alerts, dict):
        return _default_alerts(), [{"section": "alerts", "message": "must be an object"}]
    errors: List[Dict[str, Any]] = []
    alerts = normalize_alerts(raw_alerts)
    try:
        debounce = int(raw_alerts.get("debounce_seconds", DEFAULT_ALERT_DEBOUNCE_SECONDS))
        if debounce < 0:
            errors.append(
                {"section": "alerts", "field": "debounce_seconds", "message": "must be >= 0"}
            )
    except (TypeError, ValueError):
        errors.append({"section": "alerts", "field": "debounce_seconds", "message": "must be an integer"})

    for channel in ALERT_CHANNELS:
        channel_raw = raw_alerts.get(channel, {})
        if not isinstance(channel_raw, dict):
            errors.append({"section": "alerts", "field": channel, "message": "must be an object"})
            continue
        target = channel_raw.get("target", "")
        if target is not None and not isinstance(target, str):
            errors.append(
                {
                    "section": "alerts",
                    "field": f"{channel}.target",
                    "message": "must be a string",
                }
            )
    return alerts, errors


def generate_entry_id() -> str:
    return uuid.uuid4().hex


def with_persistent_ids(entries: Any) -> List[Dict[str, Any]]:
    if not isinstance(entries, list):
        return []
    normalized: List[Dict[str, Any]] = []
    seen_ids: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        item = dict(entry)
        raw_id = str(item.get("id", "") or "").strip()
        entry_id = raw_id if raw_id and raw_id not in seen_ids else generate_entry_id()
        item["id"] = entry_id
        seen_ids.add(entry_id)
        normalized.append(item)
    return normalized


def read_config_raw() -> Dict[str, Any]:
    ensure_directories()
    if CONFIG_PATH.exists():
        with CONFIG_PATH.open("r", encoding="utf-8") as handle:
            data = json.load(handle)
    else:
        data = deepcopy(DEFAULT_CONFIG)
    return {
        "tiles": data.get("tiles", []),
        "devices": with_persistent_ids(data.get("devices", [])),
        "services": with_persistent_ids(data.get("services", [])),
        "tile_refresh_hours": data.get("tile_refresh_hours", DEFAULT_TILE_REFRESH_HOURS),
        "alerts": normalize_alerts(data.get("alerts")),
    }


def load_config() -> Dict[str, Any]:
    data = read_config_raw()
    tile_refresh_hours = data.get("tile_refresh_hours", DEFAULT_TILE_REFRESH_HOURS)
    if not isinstance(tile_refresh_hours, (int, float)) or tile_refresh_hours <= 0:
        tile_refresh_hours = DEFAULT_TILE_REFRESH_HOURS
    data = {
        "tiles": normalize_tiles(data.get("tiles", [])),
        "devices": data.get("devices", []),
        "services": data.get("services", []),
        "tile_refresh_hours": tile_refresh_hours,
        "alerts": normalize_alerts(data.get("alerts")),
    }
    save_config(data)
    return data


def save_config(config: Dict[str, Any]) -> None:
    ensure_directories()
    config = {
        **config,
        "devices": with_persistent_ids(config.get("devices", [])),
        "services": with_persistent_ids(config.get("services", [])),
    }
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
        "tile_refresh_hours": config.get("tile_refresh_hours", DEFAULT_TILE_REFRESH_HOURS),
        "alerts": normalize_alerts(config.get("alerts")),
    }


@app.route("/")
def index() -> str:
    config = serialize_config(load_config())
    tiles = config["tiles"]
    devices = config["devices"]
    services = config["services"]
    tile_refresh_hours = config["tile_refresh_hours"]
    alerts = config["alerts"]
    return render_template(
        "index.html",
        tiles_json=json.dumps(tiles),
        devices_json=json.dumps(devices),
        services_json=json.dumps(services),
        tile_refresh_hours=tile_refresh_hours,
        alerts_json=json.dumps(alerts),
    )


def _load_previous_snapshot() -> Optional[Dict[str, Any]]:
    entries = load_status_history(1)
    if not entries:
        return None
    return entries[-1]


def _extract_status_map(snapshot: Optional[Dict[str, Any]], key: str) -> Dict[str, bool]:
    if not snapshot:
        return {}
    status_map: Dict[str, bool] = {}
    for item in snapshot.get(key, []):
        item_key = str(item.get("id") or item.get("name") or "").strip()
        if not item_key:
            continue
        status_map[item_key] = bool(item.get("online"))
    return status_map


def collect_transition_events(
    previous_snapshot: Optional[Dict[str, Any]], current_snapshot: Dict[str, Any], checked_at: float
) -> List[Dict[str, Any]]:
    events: List[Dict[str, Any]] = []
    checked_iso = datetime.fromtimestamp(checked_at, tz=timezone.utc).isoformat()
    for section in ("devices", "services"):
        previous = _extract_status_map(previous_snapshot, section)
        for item in current_snapshot.get(section, []):
            item_key = str(item.get("id") or item.get("name") or "").strip()
            if not item_key:
                continue
            current_online = bool(item.get("online"))
            if item_key not in previous:
                continue
            previous_online = previous[item_key]
            if previous_online == current_online:
                continue
            events.append(
                {
                    "type": "status_transition",
                    "section": section[:-1],
                    "id": item.get("id", ""),
                    "name": item.get("name", ""),
                    "from_online": previous_online,
                    "to_online": current_online,
                    "timestamp": checked_iso,
                    "epoch": checked_at,
                }
            )
    return events


def _post_alert(channel: str, target: str, event: Dict[str, Any]) -> None:
    text = (
        f"[{event['timestamp']}] {event['section']} {event.get('name') or event.get('id') or 'unknown'} "
        f"changed from {'online' if event['from_online'] else 'offline'} to {'online' if event['to_online'] else 'offline'}"
    )
    if channel == "webhook":
        requests.post(target, json=event, timeout=5)
    elif channel == "ntfy":
        requests.post(target, data=text.encode("utf-8"), timeout=5)
    elif channel in {"slack", "discord"}:
        requests.post(target, json={"text": text}, timeout=5)
    elif channel == "email":
        app.logger.info("Alert(email) target=%s payload=%s", target, event)


def emit_alert_events(events: List[Dict[str, Any]], alerts: Dict[str, Any]) -> None:
    if not events:
        return
    debounce_seconds = int(alerts.get("debounce_seconds", DEFAULT_ALERT_DEBOUNCE_SECONDS))
    now = time.time()
    for event in events:
        identity = f"{event.get('section')}:{event.get('id') or event.get('name')}:{event.get('to_online')}"
        last_sent = ALERT_LAST_EMITTED.get(identity)
        if last_sent is not None and now - last_sent < debounce_seconds:
            continue
        delivered = False
        for channel in ALERT_CHANNELS:
            channel_config = alerts.get(channel, {})
            if not isinstance(channel_config, dict):
                continue
            if not channel_config.get("enabled"):
                continue
            target = str(channel_config.get("target", "") or "").strip()
            if not target:
                continue
            try:
                _post_alert(channel, target, event)
                delivered = True
            except requests.RequestException:
                app.logger.warning("Alert delivery failed for %s", channel)
        if delivered:
            ALERT_LAST_EMITTED[identity] = now


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


def _parse_service_timeout(raw_timeout: Any) -> tuple[float, bool]:
    if raw_timeout is None:
        return 2.0, True
    try:
        timeout = float(raw_timeout)
    except (TypeError, ValueError):
        return 2.0, False
    return timeout, timeout > 0


def _parse_service_expected_status(raw_status: Any) -> tuple[int, bool]:
    if raw_status is None:
        return 200, True
    try:
        status = int(raw_status)
    except (TypeError, ValueError):
        return 200, False
    return status, 100 <= status <= 599


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
        {
            "id": device.get("id", ""),
            "name": device["name"],
            "address": device["address"],
            "online": ping_device(device["address"]),
        }
        for device in config["devices"]
    ]
    service_statuses = []
    for service in config["services"]:
        result = check_service(service)
        service_statuses.append(
            {
                "id": service.get("id", ""),
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
    config = read_config_raw()
    snapshot = build_status_snapshot(config)
    checked_at = time.time()
    with STATUS_LOCK:
        STATUS_CACHE["devices"] = snapshot["devices"]
        STATUS_CACHE["services"] = snapshot["services"]
        STATUS_CACHE["last_checked"] = checked_at
        append_status_history(snapshot, checked_at)


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


def append_status_history(snapshot: Dict[str, Any], checked_at: float) -> None:
    ensure_directories()
    previous_snapshot = _load_previous_snapshot()
    record = {
        "timestamp": datetime.fromtimestamp(checked_at, tz=timezone.utc).isoformat(),
        "epoch": checked_at,
        "devices": snapshot.get("devices", []),
        "services": snapshot.get("services", []),
    }
    with STATUS_HISTORY_PATH.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record))
        handle.write("\n")
    trim_status_history(STATUS_HISTORY_LIMIT)
    config = read_config_raw()
    events = collect_transition_events(previous_snapshot, snapshot, checked_at)
    emit_alert_events(events, config.get("alerts", {}))


def trim_status_history(max_entries: int) -> None:
    if not STATUS_HISTORY_PATH.exists():
        return
    try:
        with STATUS_HISTORY_PATH.open("r", encoding="utf-8") as handle:
            lines = handle.readlines()
    except OSError:
        return
    if len(lines) <= max_entries:
        return
    keep = lines[-max_entries:]
    with STATUS_HISTORY_PATH.open("w", encoding="utf-8") as handle:
        handle.writelines(keep)


def load_status_history(limit: int) -> List[Dict[str, Any]]:
    if not STATUS_HISTORY_PATH.exists() or limit <= 0:
        return []
    try:
        with STATUS_HISTORY_PATH.open("r", encoding="utf-8") as handle:
            lines = handle.readlines()
    except OSError:
        return []
    entries: List[Dict[str, Any]] = []
    for line in lines[-limit:]:
        line = line.strip()
        if not line:
            continue
        try:
            entries.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return entries


@app.route("/previews/<path:filename>")
def previews(filename: str):
    preview_name = Path(filename).name
    preview_path = PREVIEW_DIR / preview_name
    if preview_path.exists():
        return send_from_directory(PREVIEW_DIR, preview_name)

    config = read_config_raw()
    for tile in config.get("tiles", []):
        url = str(tile.get("url", "") or "").strip()
        raw_preview = str(tile.get("preview", "") or "").strip()
        tile_preview = Path(raw_preview).name if raw_preview else preview_filename(url)
        if tile_preview == preview_name and url:
            enqueue_preview_fetch(url, preview_name)
            break
    return send_from_directory(app.static_folder or "static", PLACEHOLDER_PREVIEW)


@app.route("/api/config", methods=["GET", "POST"])
def config():
    if request.method == "GET":
        return jsonify(serialize_config(load_config()))

    payload = request.get_json(silent=True) or {}
    current = load_config()
    tiles = payload.get("tiles", current["tiles"])
    devices = payload.get("devices", current["devices"])
    services = payload.get("services", current["services"])
    alerts = payload.get("alerts", current.get("alerts", _default_alerts()))
    validated_tiles, tile_errors = _validate_tiles(tiles)
    validated_devices, device_errors = _validate_devices(devices)
    validated_services, service_errors = _validate_services(services)
    validated_alerts, alert_errors = _validate_alerts(alerts)
    errors = tile_errors + device_errors + service_errors + alert_errors
    if errors:
        return jsonify({"message": "Invalid configuration", "errors": errors}), 400

    updated = {
        "tiles": normalize_tiles(validated_tiles),
        "devices": validated_devices,
        "services": validated_services,
        "tile_refresh_hours": current.get("tile_refresh_hours", DEFAULT_TILE_REFRESH_HOURS),
        "alerts": validated_alerts,
    }
    save_config(updated)
    return jsonify(serialize_config(updated))


@app.route("/api/status")
def status():
    return jsonify(status_payload())


@app.route("/api/status/history")
def status_history():
    raw_limit = request.args.get("limit", type=int)
    limit = raw_limit if raw_limit is not None else 120
    if limit < 1:
        limit = 1
    if limit > STATUS_HISTORY_LIMIT:
        limit = STATUS_HISTORY_LIMIT
    history = load_status_history(limit)
    return jsonify({"entries": history, "limit": limit, "count": len(history)})


@app.before_request
def ensure_scheduler_started() -> None:
    start_status_scheduler()
    start_preview_worker()


if __name__ == "__main__":
    start_status_scheduler()
    start_preview_worker()
    app.run(host="0.0.0.0", port=5000, debug=True)
