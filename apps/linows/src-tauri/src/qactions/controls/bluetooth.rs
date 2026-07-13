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
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, InfoValue, SystemControl};
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

/// `a{oa{sa{sv}}}` - the BlueZ object tree from `GetManagedObjects`.
type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

/// The BlueZ facts the panel needs, from one `GetManagedObjects` call.
struct Snapshot {
    adapter_path: OwnedObjectPath,
    powered: bool,
    /// Aliases of currently connected devices.
    connected: Vec<String>,
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
            Ok(s) if s.connected.is_empty() => InfoValue::Text {
                text: "On, not connected".to_string(),
            },
            // One row per connected device: a comma-joined line gets unreadable
            // (and wraps mid-word) once more than a couple are paired.
            Ok(s) => InfoValue::List { items: s.connected },
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

    let mut connected: Vec<String> = objects
        .values()
        .filter_map(|ifaces| ifaces.get(DEVICE_IFACE))
        .filter(|props| prop_bool(props, "Connected").unwrap_or(false))
        .filter_map(|props| prop_str(props, "Alias").or_else(|| prop_str(props, "Address")))
        .collect();
    connected.sort();

    Ok(Snapshot {
        adapter_path,
        powered,
        connected,
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
