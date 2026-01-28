from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from typing import List

import requests
from flask import Flask, jsonify, render_template

app = Flask(__name__)


@dataclass
class Tile:
    title: str
    url: str


@dataclass
class Device:
    name: str
    address: str


@dataclass
class Service:
    name: str
    url: str


WEB_TILES: List[Tile] = [
    Tile(title="Docs", url="https://docs.python.org/3/"),
    Tile(title="Grafana", url="https://grafana.com/"),
    Tile(title="Jellyfin", url="https://jellyfin.org/"),
    Tile(title="Kavita", url="https://www.kavitareader.com/"),
]

DEVICES: List[Device] = [
    Device(name="Router", address="192.168.1.1"),
    Device(name="NAS", address="192.168.1.10"),
    Device(name="Desktop", address="192.168.1.20"),
]

SERVICES: List[Service] = [
    Service(name="Jellyfin", url="https://demo.jellyfin.org/stable"),
    Service(name="Kavita", url="https://www.kavitareader.com/"),
    Service(name="Calibre", url="https://calibre-ebook.com/"),
]


@app.route("/")
def index() -> str:
    tiles = [tile.__dict__ for tile in WEB_TILES]
    devices = [device.__dict__ for device in DEVICES]
    services = [service.__dict__ for service in SERVICES]
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


@app.route("/api/status")
def status():
    device_statuses = [
        {"name": device.name, "address": device.address, "online": ping_device(device.address)}
        for device in DEVICES
    ]
    service_statuses = [
        {"name": service.name, "url": service.url, "online": check_service(service.url)}
        for service in SERVICES
    ]
    return jsonify({"devices": device_statuses, "services": service_statuses})


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, debug=True)
