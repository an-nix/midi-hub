use btleplug::api::{
    Central, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::stream::StreamExt;
use midir::MidiOutput;
use midir::os::unix::VirtualOutput;
use std::error::Error;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;
use zbus::{dbus_interface, ConnectionBuilder, Connection};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::Deserialize;
use std::fs;

const MIDI_SERVICE_UUID: Uuid = Uuid::from_u128(0x03b80e5a_ede8_4b33_a751_6ce34ec4c700);
const MIDI_CHAR_UUID: Uuid = Uuid::from_u128(0x7772e5db_3868_4112_a1a9_f2669d106bf3);

#[derive(Debug, Deserialize)]
struct Config {
    /// Single device MAC (backwards compatibility)
    device_mac: Option<String>,
    /// Multiple device MACs to manage (preferred)
    device_macs: Option<Vec<String>>,
    /// Optional ALSA port name (defaults to "BLE-MIDI-Bridge")
    alsa_port: Option<String>,
}

fn load_config() -> Result<Config, Box<dyn Error>> {
    // Search common locations: /etc/midi-hub/config.json, ./config.json
    let candidates = ["/etc/midi-hub/config.json", "./config.json"];

    for path in &candidates {
        if let Ok(s) = fs::read_to_string(path) {
            let cfg: Config = serde_json::from_str(&s)?;
            log::info!("Loaded config from {}", path);
            return Ok(cfg);
        }
    }

    // If none found, try to read a bundled sample (project root)
    if let Ok(s) = fs::read_to_string("config.json") {
        let cfg: Config = serde_json::from_str(&s)?;
        log::info!("Loaded config from ./config.json");
        return Ok(cfg);
    }

    Err("No config.json found in /etc/midi-hub or current directory".into())
}

// BlueZ Agent for auto-authorizing pairing
struct Agent1;

#[dbus_interface(name = "org.bluez.Agent1")]
impl Agent1 {
    async fn release(&self) -> zbus::fdo::Result<()> {
        log::info!("Agent released");
        Ok(())
    }

    async fn request_pin_code(&self, device: zbus::zvariant::OwnedObjectPath) -> zbus::fdo::Result<String> {
        log::info!("PIN code requested for device: {}", device);
        Ok("0000".to_string())
    }

    async fn display_pin_code(&self, device: zbus::zvariant::OwnedObjectPath, pincode: String) -> zbus::fdo::Result<()> {
        log::info!("Display PIN {} for device: {}", pincode, device);
        Ok(())
    }

    async fn request_passkey(&self, device: zbus::zvariant::OwnedObjectPath) -> zbus::fdo::Result<u32> {
        log::info!("Passkey requested for device: {}", device);
        Ok(0)
    }

    async fn display_passkey(&self, device: zbus::zvariant::OwnedObjectPath, passkey: u32, entered: u16) -> zbus::fdo::Result<()> {
        log::info!("Display passkey {} (entered: {}) for device: {}", passkey, entered, device);
        Ok(())
    }

    async fn request_confirmation(&self, device: zbus::zvariant::OwnedObjectPath, passkey: u32) -> zbus::fdo::Result<()> {
        log::info!("Auto-confirming passkey {} for device: {}", passkey, device);
        Ok(())
    }

    async fn request_authorization(&self, device: zbus::zvariant::OwnedObjectPath) -> zbus::fdo::Result<()> {
        log::info!("Auto-authorizing pairing for device: {}", device);
        Ok(())
    }

    async fn authorize_service(&self, device: zbus::zvariant::OwnedObjectPath, uuid: String) -> zbus::fdo::Result<()> {
        log::info!("Auto-authorizing service {} for device: {}", uuid, device);
        Ok(())
    }

    async fn cancel(&self) -> zbus::fdo::Result<()> {
        log::info!("Agent request cancelled");
        Ok(())
    }
}

async fn register_agent() -> Result<Connection, Box<dyn Error>> {
    log::info!("Registering BlueZ agent...");
    
    let agent = Agent1;
    let conn = ConnectionBuilder::system()?
        .serve_at("/org/bluez/agent", agent)?
        .build()
        .await?;

    // Register agent with BlueZ AgentManager
    let proxy = zbus::Proxy::new(
        &conn,
        "org.bluez",
        "/org/bluez",
        "org.bluez.AgentManager1",
    ).await?;

    let agent_path = zbus::zvariant::ObjectPath::from_str_unchecked("/org/bluez/agent");
    
    proxy.call_method(
        "RegisterAgent",
        &(&agent_path, "NoInputNoOutput"),
    ).await?;

    proxy.call_method(
        "RequestDefaultAgent",
        &(&agent_path,),
    ).await?;

    log::info!("Agent registered successfully");
    
    Ok(conn)
}

async fn pair_trust_device_via_dbus(device_mac: &str) -> Result<(), Box<dyn Error>> {
    let conn = zbus::Connection::system().await?;
    
    // Convert MAC to BlueZ object path
    let device_path_str = format!("/org/bluez/hci0/dev_{}", device_mac.replace(":", "_"));
    let device_path = zbus::zvariant::ObjectPath::try_from(device_path_str.as_str())?;
    
    log::info!("Pairing device via D-Bus: {}", device_path);
    
    // Call Device1.Pair
    let device_proxy = zbus::Proxy::new(
        &conn,
        "org.bluez",
        device_path.clone(),
        "org.bluez.Device1",
    ).await?;
    
    match device_proxy.call_method("Pair", &()).await {
        Ok(_) => log::info!("Pairing successful"),
        Err(e) => log::warn!("Pairing failed (may already be paired): {}", e),
    }
    
    // Wait a bit for pairing to complete
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    log::info!("Setting device as trusted...");
    
    // Set Trusted property to true
    let props_proxy = zbus::Proxy::new(
        &conn,
        "org.bluez",
        device_path,
        "org.freedesktop.DBus.Properties",
    ).await?;
    
    props_proxy.call_method(
        "Set",
        &("org.bluez.Device1", "Trusted", zbus::zvariant::Value::new(true)),
    ).await?;
    
    log::info!("Device paired and trusted via D-Bus");
    
    Ok(())
}

struct AlsaBridge {
    conn_out: midir::MidiOutputConnection,
}

impl AlsaBridge {
    fn new(port_name: &str) -> Result<Self, Box<dyn Error>> {
        let midi_out = MidiOutput::new(port_name)?;
        let conn_out = midi_out.create_virtual(port_name)?;
        log::info!("Created ALSA virtual port: {}", port_name);
        Ok(AlsaBridge { conn_out })
    }

    fn send_midi(&mut self, data: &[u8]) {
        if data.len() >= 3 {
            // Skip BLE MIDI header (first 2 bytes: header + timestamp)
            let midi_data = &data[2..];
            if let Err(e) = self.conn_out.send(midi_data) {
                log::error!("Failed to send MIDI: {}", e);
            } else {
                log::info!("MIDI event: {:02x?}", midi_data);
            }
        }
    }
}

async fn find_device(adapter: &Adapter, device_mac: &str) -> Result<Peripheral, Box<dyn Error>> {
    log::info!("Scanning for device {}...", device_mac);

    adapter.start_scan(ScanFilter::default()).await?;
    
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        for peripheral in adapter.peripherals().await? {
            if let Ok(Some(props)) = peripheral.properties().await {
                let addr_str = format!("{:?}", props.address);
                log::debug!("Found device: {} - Address: {}", 
                    props.local_name.as_deref().unwrap_or("Unknown"), 
                    addr_str);
                
                     // Check if address contains our MAC
                     if addr_str.to_uppercase().contains(&device_mac.replace(":", "_").to_uppercase()) ||
                         addr_str.to_uppercase().contains(&device_mac.to_uppercase()) {
                    log::info!("Matched device: {} ({})", 
                        props.local_name.as_deref().unwrap_or("Unknown"),
                        addr_str);
                    adapter.stop_scan().await?;
                    return Ok(peripheral);
                }
            }
        }
        time::sleep(Duration::from_millis(500)).await;
    }
    
    adapter.stop_scan().await.ok();
    Err("Device not found".into())
}

async fn connect_and_forward(
    peripheral: &Peripheral,
    alsa: Arc<Mutex<AlsaBridge>>,
) -> Result<(), Box<dyn Error>> {
    log::info!("Connecting to device...");
    
    // Connect (this may trigger pairing/authorization on the device side)
    log::info!("Calling Connect (may trigger pairing/authorization on device)");
    peripheral.connect().await?;
    
    log::info!("Discovering services...");
    peripheral.discover_services().await?;
    
    // Find MIDI characteristic
    let chars = peripheral.characteristics();
    let midi_char = chars
        .iter()
        .find(|c| c.uuid == MIDI_CHAR_UUID)
        .ok_or("MIDI characteristic not found")?;
    
    log::info!("Found MIDI characteristic, subscribing to notifications...");
    peripheral.subscribe(midi_char).await?;
    
    // Get notification stream
    let mut notification_stream = peripheral.notifications().await?;
    
    log::info!("Connected and forwarding MIDI data...");
    
    // Monitor connection state
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let device_clone = peripheral.clone();
    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(1)).await;
            if !device_clone.is_connected().await.unwrap_or(false) {
                log::warn!("Device disconnected (monitor)");
                let _ = tx.send(()).await;
                break;
            }
        }
    });
    
    loop {
        tokio::select! {
            Some(data) = notification_stream.next() => {
                // Lock ALSA bridge and send MIDI
                let mut guard = alsa.lock().await;
                guard.send_midi(&data.value);
            }
            _ = rx.recv() => {
                log::warn!("Connection lost");
                break;
            }
        }
    }
    
    if peripheral.is_connected().await? {
        peripheral.disconnect().await?;
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    log::info!("Starting BLE-MIDI Bridge (Rust)...");
    
    // Register agent first to handle pairing
    let _agent_conn = match register_agent().await {
        Ok(conn) => {
            log::info!("Agent registered, ready to handle authorization requests");
            Some(conn)
        }
        Err(e) => {
            log::warn!("Failed to register agent: {}, continuing anyway...", e);
            None
        }
    };
    
    // Load config (try /etc/midi-hub/config.json, then ./config.json)
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load config.json: {}. Falling back to default MAC 52:17:32:38:20:BF", e);
            Config { device_mac: Some("52:17:32:38:20:BF".to_string()), device_macs: None, alsa_port: None }
        }
    };

    let port_name = config.alsa_port.as_deref().unwrap_or("BLE-MIDI-Bridge");
    let alsa = AlsaBridge::new(port_name)?;
    let alsa = Arc::new(Mutex::new(alsa));

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters.into_iter().next().ok_or("No Bluetooth adapter found")?;

    // Build a list of device MACs from config (support both legacy `device_mac` and `device_macs`)
    let mut devices: Vec<String> = Vec::new();
    if let Some(v) = config.device_macs {
        devices.extend(v.into_iter());
    }
    if let Some(s) = config.device_mac {
        devices.push(s);
    }
    if devices.is_empty() {
        devices.push("52:17:32:38:20:BF".to_string());
    }

    log::info!("Managing {} device(s): {:?}", devices.len(), devices);

    // Spawn one task per device to manage scanning, pairing and forwarding
    for dev in devices.into_iter() {
        let adapter_clone = adapter.clone();
        let alsa_clone = alsa.clone();
        tokio::spawn(async move {
            loop {
                match find_device(&adapter_clone, &dev).await {
                    Ok(peripheral) => {
                        log::info!("[{}] Device found, attempting pairing and connection...", dev);
                        if let Err(e) = pair_trust_device_via_dbus(&dev).await {
                            log::error!("[{}] Failed to pair/trust via D-Bus: {}", dev, e);
                        }

                        if let Err(e) = connect_and_forward(&peripheral, alsa_clone.clone()).await {
                            log::error!("[{}] Connection error: {}", dev, e);
                        }
                        log::info!("[{}] Disconnected, waiting 5s before reconnect...", dev);
                        time::sleep(Duration::from_secs(5)).await;
                    }
                    Err(e) => {
                        log::error!("[{}] Discovery error: {}, retrying in 5s...", dev, e);
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }

    // Keep the main task alive indefinitely
    futures::future::pending::<()>().await;
}
