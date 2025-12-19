#!/bin/bash
# Script pour forcer l'appairage et la connexion du FootCtrlPlus

echo "Removing old pairing data..."
bluetoothctl remove 52:17:32:38:20:BF 2>/dev/null

sleep 2

echo "Starting scan and pairing process..."
echo "Please accept any pairing request on the FootCtrlPlus if prompted."

# Use expect-like behavior with bluetoothctl
{
    echo "power on"
    sleep 1
    echo "agent NoInputNoOutput"
    sleep 1
    echo "default-agent"
    sleep 1
    echo "scan on"
    sleep 8
    echo "scan off"
    sleep 1
    echo "trust 52:17:32:38:20:BF"
    sleep 1
    echo "pair 52:17:32:38:20:BF"
    sleep 5
    echo "connect 52:17:32:38:20:BF"
    sleep 5
    echo "info 52:17:32:38:20:BF"
    sleep 2
    echo "quit"
} | bluetoothctl

echo ""
echo "Connection attempt complete. Check status above."
echo "If connected, you can now run: ./target/release/ble-midi-bridge"
