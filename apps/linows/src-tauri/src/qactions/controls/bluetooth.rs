//! REFERENCE ADAPTER - copy this file to add a new system control.
//!
//! A control implements `SystemControl` and keeps ALL of its OS-specific code
//! (D-Bus, CLIs, syscalls) inside itself. To add your own:
//!
//!   1. Copy this file and rename the type (e.g. `WifiControl`).
//!   2. Implement `state()` (+ `info()` if the descriptor declares fields).
//!   3. Implement `apply(_:)` - perform the change, return an `ActionOutcome`.
//!   4. Register it in `qactions::adapter` under your action id.
//!   5. Declare the descriptor + result binding in the shared `core/qactions`.
//!
//! Nothing else (panel, keyboard, rendering) changes. That is the whole point.
//!
//! This adapter talks to BlueZ over the system bus rather than shelling out to
//! `bluetoothctl`: the CLI is just a client of the same D-Bus API, its output
//! is not a stable interface, and spawning host tools from the AppImage needs
//! LD_LIBRARY_PATH scrubbing that an in-process call avoids entirely.

use crate::platform::linux::dbus;
use crate::qactions::{
    ActionIntent, ActionOutcome, ActionState, InfoValue, ListItem, SystemControl,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const BLUEZ_DEST: &str = "org.bluez";
const OBJECT_MANAGER_IFACE: &str = "org.freedesktop.DBus.ObjectManager";
const ADAPTER_IFACE: &str = "org.bluez.Adapter1";
const DEVICE_IFACE: &str = "org.bluez.Device1";

const NO_SERVICE: &str = "Bluetooth service not running";
const NO_ADAPTER: &str = "Bluetooth hardware not found";

/// How long to wait for the controller to apply a power change, and how often
/// to re-check while waiting. Mirrors the macOS reference adapter: without the
/// settle wait, the panel's immediate re-read can still see the old value.
const SETTLE_TIMEOUT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// How long to wait for a device Connect/Disconnect before reporting failure.
/// BlueZ's own call can hang up to ~25s when a device is off or out of range;
/// 6s is plenty for a nearby device and keeps the shared D-Bus runtime from
/// stalling other calls (e.g. window focus) for long.
const DEVICE_ACTION_TIMEOUT: Duration = Duration::from_secs(6);

/// `a{oa{sa{sv}}}` - the BlueZ object tree from `GetManagedObjects`.
type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

/// A paired device from the BlueZ tree.
struct Device {
    path: OwnedObjectPath,
    alias: String,
    connected: bool,
}

/// The BlueZ facts the panel needs, from one `GetManagedObjects` call.
struct Snapshot {
    adapter_path: OwnedObjectPath,
    powered: bool,
    /// Paired devices, connected ones first, for the interactive device list.
    devices: Vec<Device>,
}

/// Toggles and reports Linux system Bluetooth power. Action id: `"bluetooth"`.
pub struct BluetoothControl;

impl SystemControl for BluetoothControl {
    fn state(&self) -> ActionState {
        match snapshot() {
            Ok(s) if s.powered => ActionState::On,
            Ok(_) => ActionState::Off,
            Err(reason) => ActionState::Unavailable { reason },
        }
    }

    fn info(&self, keys: &[String]) -> HashMap<String, InfoValue> {
        let mut values = HashMap::new();
        if !keys.iter().any(|k| k == "status") {
            return values;
        }
        let value = match snapshot() {
            Ok(s) if !s.powered => InfoValue::Text {
                text: "Off".to_string(),
            },
            Ok(s) if s.devices.is_empty() => InfoValue::Text {
                text: "On, no paired devices".to_string(),
            },
            // One clickable row per paired device (connect/disconnect); a
            // comma-joined line gets unreadable once more than a couple pair.
            Ok(s) => InfoValue::List {
                items: s
                    .devices
                    .into_iter()
                    .map(|d| ListItem {
                        id: Some(d.path.to_string()),
                        label: d.alias,
                        on: Some(d.connected),
                    })
                    .collect(),
            },
            Err(reason) => InfoValue::Unavailable { reason },
        };
        values.insert("status".to_string(), value);
        values
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        // Re-read right before acting so a change made elsewhere between show
        // and press is not clobbered.
        let snapshot = match snapshot() {
            Ok(s) => s,
            Err(reason) => return ActionOutcome::Failed { message: reason },
        };
        let target = match intent {
            ActionIntent::Toggle => !snapshot.powered,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Bluetooth has no run action".to_string(),
                };
            }
        };

        if let Err(message) = set_powered(&snapshot.adapter_path, target) {
            return ActionOutcome::Failed { message };
        }
        // bluetoothd applies the change asynchronously; wait until the power
        // state reflects the target so the panel's re-read is truthful.
        if !wait_for_power_state(&snapshot.adapter_path, target) {
            return ActionOutcome::Failed {
                message: format!("Could not turn Bluetooth {}", on_off(target)),
            };
        }
        ActionOutcome::Ok {
            banner: Some(format!("Bluetooth {}", on_off(target))),
        }
    }

    fn apply_item(&self, item_id: &str, intent: ActionIntent) -> ActionOutcome {
        if !matches!(intent, ActionIntent::Toggle) {
            return ActionOutcome::Failed {
                message: "Devices can only be connected or disconnected".to_string(),
            };
        }
        // item_id is a BlueZ device object path. Re-read its live connection so
        // a click flips the real state (not a stale rendered one) and gives us
        // a name for the banner.
        let Some((connected, alias)) = device_state(item_id) else {
            return ActionOutcome::Failed {
                message: "Device is no longer available".to_string(),
            };
        };
        let connect = !connected;
        let (verb_ok, verb_fail) = if connect {
            ("Connected to", "connect to")
        } else {
            ("Disconnected from", "disconnect from")
        };
        if set_device_connected(item_id, connect) {
            ActionOutcome::Ok {
                banner: Some(format!("{verb_ok} {alias}")),
            }
        } else {
            ActionOutcome::Failed {
                message: format!("Failed to {verb_fail} {alias}"),
            }
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// One `GetManagedObjects` call -> adapter path, power state, connected
/// devices. `Err` carries the human reason shown as unavailable.
fn snapshot() -> Result<Snapshot, String> {
    let Some(conn) = dbus::system() else {
        return Err(NO_SERVICE.to_string());
    };
    let objects: ManagedObjects = dbus::runtime()
        .block_on(async {
            conn.call_method(
                Some(BLUEZ_DEST),
                "/",
                Some(OBJECT_MANAGER_IFACE),
                "GetManagedObjects",
                &(),
            )
            .await?
            .body()
            .deserialize()
        })
        .map_err(|_| NO_SERVICE.to_string())?;

    let (adapter_path, adapter_props) = objects
        .iter()
        .find_map(|(path, ifaces)| Some((path.clone(), ifaces.get(ADAPTER_IFACE)?)))
        .ok_or_else(|| NO_ADAPTER.to_string())?;
    let powered = prop_bool(adapter_props, "Powered").unwrap_or(false);

    let mut devices: Vec<Device> = objects
        .iter()
        .filter_map(|(path, ifaces)| {
            let props = ifaces.get(DEVICE_IFACE)?;
            // Only devices we have paired with - the ones a user can re-connect.
            if !prop_bool(props, "Paired").unwrap_or(false) {
                return None;
            }
            Some(Device {
                path: path.clone(),
                alias: prop_str(props, "Alias").or_else(|| prop_str(props, "Address"))?,
                connected: prop_bool(props, "Connected").unwrap_or(false),
            })
        })
        .collect();
    // Connected first, then alphabetical, so the active devices lead the list.
    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.alias.to_lowercase().cmp(&b.alias.to_lowercase()))
    });

    Ok(Snapshot {
        adapter_path,
        powered,
        devices,
    })
}

/// Read one device's live `Connected` state and display name by object path.
fn device_state(path: &str) -> Option<(bool, String)> {
    let conn = dbus::system()?;
    dbus::runtime().block_on(async {
        let proxy = zbus::Proxy::new(conn, BLUEZ_DEST, path, DEVICE_IFACE)
            .await
            .ok()?;
        let connected: bool = proxy.get_property("Connected").await.ok()?;
        let alias = proxy
            .get_property::<String>("Alias")
            .await
            .unwrap_or_else(|_| "device".to_string());
        Some((connected, alias))
    })
}

/// Call `Connect`/`Disconnect` on a device, bounded by `DEVICE_ACTION_TIMEOUT`.
/// Returns whether it completed within the window.
fn set_device_connected(path: &str, connect: bool) -> bool {
    let Some(conn) = dbus::system() else {
        return false;
    };
    let method = if connect { "Connect" } else { "Disconnect" };
    dbus::runtime().block_on(async {
        let Ok(proxy) = zbus::Proxy::new(conn, BLUEZ_DEST, path, DEVICE_IFACE).await else {
            return false;
        };
        matches!(
            tokio::time::timeout(DEVICE_ACTION_TIMEOUT, proxy.call_method(method, &())).await,
            Ok(Ok(_))
        )
    })
}

fn prop_bool(props: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    props.get(key)?.downcast_ref::<bool>().ok()
}

fn prop_str(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value: zbus::zvariant::Str = props.get(key)?.downcast_ref().ok()?;
    Some(value.to_string())
}

fn set_powered(adapter_path: &OwnedObjectPath, on: bool) -> Result<(), String> {
    let Some(conn) = dbus::system() else {
        return Err(NO_SERVICE.to_string());
    };
    dbus::runtime()
        .block_on(async {
            zbus::Proxy::new(conn, BLUEZ_DEST, adapter_path.as_str(), ADAPTER_IFACE)
                .await?
                .set_property("Powered", on)
                .await
        })
        .map_err(|err| friendly_set_error(&err, on))
}

fn friendly_set_error(err: &zbus::fdo::Error, target: bool) -> String {
    // BlueZ rejects the write with org.bluez.Error.Blocked when rfkill has the
    // radio soft-blocked; that is not an fdo error, so it arrives ZBus-wrapped.
    if let zbus::fdo::Error::ZBus(zbus::Error::MethodError(name, _, _)) = err
        && name.as_str() == "org.bluez.Error.Blocked"
    {
        return "Bluetooth is blocked by rfkill".to_string();
    }
    format!("Could not turn Bluetooth {}", on_off(target))
}

/// Polls the adapter's `Powered` property until it reaches `target` or the
/// settle timeout. Runs on the blocking pool, so plain sleeps are fine.
fn wait_for_power_state(adapter_path: &OwnedObjectPath, target: bool) -> bool {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if read_powered(adapter_path) == Some(target) {
            return true;
        }
        if Instant::now() >= deadline {
            return read_powered(adapter_path) == Some(target);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn read_powered(adapter_path: &OwnedObjectPath) -> Option<bool> {
    let conn = dbus::system()?;
    dbus::runtime()
        .block_on(async {
            zbus::Proxy::new(conn, BLUEZ_DEST, adapter_path.as_str(), ADAPTER_IFACE)
                .await?
                .get_property::<bool>("Powered")
                .await
        })
        .ok()
}
