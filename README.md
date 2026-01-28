# Raz Dashboard

Raz Dashboard is a personal homepage dashboard that surfaces quick links alongside device and service monitoring. The landing page shows tiles for common resources, plus live status indicators for home network devices and self-hosted services driven by a JSON status API.

## Features

- Quick-link tiles for frequently used sites.
- Device availability checks via ICMP ping.
- Service availability checks via HTTP requests.
- JSON status endpoint at `/api/status` for devices and services.

## Running the app

1. Create and activate a virtual environment (optional but recommended):

   ```bash
   python -m venv .venv
   source .venv/bin/activate
   ```

2. Install dependencies:

   ```bash
   pip install -r requirements.txt
   ```

3. Start the Flask app:

   ```bash
   python app.py
   ```

4. Open the app in your browser:

   ```text
   http://localhost:5000
   ```

The app binds to `0.0.0.0:5000` in development mode, so you can also reach it from other devices on the same network.
