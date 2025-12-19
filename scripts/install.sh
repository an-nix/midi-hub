#!/usr/bin/env bash
set -euo pipefail

# Simple installer for midi-hub
# - builds release
# - installs binary to /usr/local/bin
# - installs config to /etc/midi-hub/config.json (if not present)
# - installs systemd service as /etc/systemd/system/midi-hub.service
# Usage: sudo ./scripts/install.sh

INSTALL_BIN=/usr/local/bin/midi-hub
SERVICE_PATH=/etc/systemd/system/midi-hub.service
CONFIG_DIR=/etc/midi-hub

echo "Building release..."
cargo build --release

echo "Installing binary to $INSTALL_BIN"
sudo install -m 0755 target/release/midi-hub "$INSTALL_BIN"

# Install config if none exists
if [ ! -f "$CONFIG_DIR/config.json" ]; then
  echo "Installing default config to $CONFIG_DIR/config.json"
  sudo mkdir -p "$CONFIG_DIR"
  sudo cp config.json.sample "$CONFIG_DIR/config.json"
else
  echo "Config already exists at $CONFIG_DIR/config.json - leaving it untouched"
fi

# Create systemd service
sudo tee "$SERVICE_PATH" > /dev/null <<'EOF'
[Unit]
Description=midi-hub — BLE/USB MIDI bridge to a stable ALSA port
After=bluetooth.service
Requires=bluetooth.service

[Service]
Type=simple
ExecStart=/usr/local/bin/midi-hub
Restart=always
RestartSec=5
User=piamp
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now midi-hub

echo "Installation complete. Service started: sudo systemctl status midi-hub" 
