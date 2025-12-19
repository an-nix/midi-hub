# BLE MIDI to ALSA Bridge (Rust)

This application creates a stable virtual ALSA MIDI port and automatically connects to BLE MIDI devices. It bridges the BLE MIDI data to the ALSA port, ensuring that applications like PiPedal can stay connected to the ALSA port even if the BLE connection drops and reconnects.

## Features

- **Stable ALSA Port:** Creates a virtual MIDI port named "BLE-MIDI-Bridge".
- **Auto-Connect:** Automatically scans and connects to BLE MIDI devices (filtering by service UUID or name).
- **Auto-Reconnect:** Automatically reconnects if the BLE connection is lost.
- **Transparent Bridging:** Forwards MIDI messages from BLE to ALSA.

## Prerequisites

- Rust toolchain (installed automatically via the setup script or manually via rustup).
- System dependencies: `libasound2-dev`, `libdbus-1-dev`, `pkg-config`.

## Building and Running

1.  **Build the project:**
    ```bash
    cargo build --release
    ```

2.  **Run the bridge:**
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
