# midi-hub — BLE/USB MIDI bridge to a stable ALSA port

This application creates a stable virtual ALSA MIDI port and automatically connects to BLE MIDI devices. It bridges the BLE MIDI data to the ALSA port, ensuring that applications like PiPedal can stay connected to the ALSA port even if the BLE connection drops and reconnects.

## Features

- **Stable ALSA Port:** Creates a virtual MIDI port named "BLE-MIDI-Bridge".
- **Auto-Connect:** Automatically scans and connects to BLE MIDI devices (filtering by service UUID or name).
- **Auto-Reconnect:** Automatically reconnects if the BLE connection is lost.
- **Transparent Bridging:** Forwards MIDI messages from BLE to ALSA.

## Prerequisites

- Rust toolchain (installed automatically via the setup script or manually via rustup).
- System dependencies: `libasound2-dev`, `libdbus-1-dev`, `pkg-config`.

## Configuration

Create a `config.json` in `/etc/midi-hub/config.json` or in the project root. Example (`config.json.sample` included):

```
{
    "device_mac": "52:17:32:38:20:BF",
    "alsa_port": "BLE-MIDI-Bridge"
}
```

`device_mac` is the BLE device MAC address to connect to. `alsa_port` is optional and defaults to `BLE-MIDI-Bridge`.

## Building and Running

1.  **Build & Install (recommended):**
    ```bash
    # from project root
    sudo ./scripts/install.sh
    ```

2.  **Run for development (not installed):**
    ```bash
    cargo run --release
    ```

## Usage with PiPedal

1.  Start this bridge application.
2.  In PiPedal (or any other MIDI software), connect to the **"BLE-MIDI-Bridge"** ALSA port.
3.  Turn on your BLE MIDI controller. The bridge will connect to it automatically.
4.  If the controller disconnects, the bridge will reconnect automatically. PiPedal will remain connected to the bridge.

## Troubleshooting

- **Permissions:** Ensure your user is in the `bluetooth` group.
- **Logs:** Run with `RUST_LOG=info cargo run` to see more details.
