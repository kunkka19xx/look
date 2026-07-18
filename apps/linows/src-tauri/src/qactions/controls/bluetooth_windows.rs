//! Windows Bluetooth control. See docs/writing-controls.md for the design notes.

use crate::qactions::{
    ActionIntent, ActionOutcome, ActionState, InfoValue, ListItem, SystemControl,
};
use std::collections::HashMap;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::System::Com::CoIncrementMTAUsage;
use windows::core::HSTRING;

const NO_ADAPTER: &str = "Bluetooth hardware not found";
const ACCESS_DENIED: &str = "Bluetooth access is blocked by Windows";
const GONE: &str = "Device is no longer available";

const SETTLE_TIMEOUT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// A paired device row. `id` is the classic address (decimal) when the device
/// can be connected/disconnected; LE devices have no `id` and stay display-only.
struct Device {
    id: Option<String>,
    name: String,
    connected: bool,
}

pub struct BluetoothControl;

impl SystemControl for BluetoothControl {
    fn state(&self) -> ActionState {
        match bluetooth_radio() {
            Ok(radio) if radio_is_on(&radio) => ActionState::On,
            Ok(_) => ActionState::Off,
            Err(reason) => ActionState::Unavailable { reason },
        }
    }

    fn info(&self, keys: &[String]) -> HashMap<String, InfoValue> {
        let mut values = HashMap::new();
        if !keys.iter().any(|k| k == "status") {
            return values;
        }
        let value = match bluetooth_radio() {
            Ok(radio) if !radio_is_on(&radio) => InfoValue::Text {
                text: "Off".to_string(),
            },
            Ok(_) => devices_to_info(paired_devices()),
            Err(reason) => InfoValue::Unavailable { reason },
        };
        values.insert("status".to_string(), value);
        values
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        let radio = match bluetooth_radio() {
            Ok(radio) => radio,
            Err(reason) => return ActionOutcome::Failed { message: reason },
        };
        let target = match intent {
            ActionIntent::Toggle => !radio_is_on(&radio),
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Bluetooth has no run action".to_string(),
                };
            }
        };

        if let Err(message) = set_powered(&radio, target) {
            return ActionOutcome::Failed { message };
        }
        if !wait_for_power_state(&radio, target) {
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
        let Ok(address) = item_id.parse::<u64>() else {
            return ActionOutcome::Failed {
                message: GONE.to_string(),
            };
        };
        match winbt::toggle_connection(address) {
            Ok((connected, name)) => {
                let verb = if connected {
                    "Connected to"
                } else {
                    "Disconnected from"
                };
                ActionOutcome::Ok {
                    banner: Some(format!("{verb} {name}")),
                }
            }
            Err(message) => ActionOutcome::Failed { message },
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// Build the "status" info value from a device list.
fn devices_to_info(devices: Vec<Device>) -> InfoValue {
    if devices.is_empty() {
        return InfoValue::Text {
            text: "On, no paired devices".to_string(),
        };
    }
    InfoValue::List {
        items: devices
            .into_iter()
            .map(|d| ListItem {
                id: d.id,
                label: d.name,
                on: Some(d.connected),
            })
            .collect(),
    }
}

/// Keep the process in an MTA so WinRT calls on pooled blocking threads work.
fn ensure_mta() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = unsafe { CoIncrementMTAUsage() };
    });
}

fn bluetooth_radio() -> Result<Radio, String> {
    ensure_mta();
    let radios = Radio::GetRadiosAsync()
        .and_then(|op| op.get())
        .map_err(|_| NO_ADAPTER.to_string())?;
    for radio in radios {
        if radio.Kind() == Ok(RadioKind::Bluetooth) {
            return Ok(radio);
        }
    }
    Err(NO_ADAPTER.to_string())
}

fn radio_is_on(radio: &Radio) -> bool {
    radio.State() == Ok(RadioState::On)
}

fn set_powered(radio: &Radio, on: bool) -> Result<(), String> {
    let access = Radio::RequestAccessAsync()
        .and_then(|op| op.get())
        .map_err(|_| ACCESS_DENIED.to_string())?;
    if access != RadioAccessStatus::Allowed {
        return Err(ACCESS_DENIED.to_string());
    }
    let target = if on { RadioState::On } else { RadioState::Off };
    match radio.SetStateAsync(target).and_then(|op| op.get()) {
        Ok(RadioAccessStatus::Allowed) => Ok(()),
        Ok(_) => Err(ACCESS_DENIED.to_string()),
        Err(_) => Err(format!("Could not turn Bluetooth {}", on_off(on))),
    }
}

/// Poll the radio until it reports `target` or the settle timeout elapses.
fn wait_for_power_state(radio: &Radio, target: bool) -> bool {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if radio_is_on(radio) == target {
            return true;
        }
        if Instant::now() >= deadline {
            return radio_is_on(radio) == target;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Paired devices (classic + LE), connected first. Best-effort: empty on failure.
fn paired_devices() -> Vec<Device> {
    let mut devices = Vec::new();
    collect_paired(&mut devices, Transport::Classic);
    collect_paired(&mut devices, Transport::LowEnergy);

    // Drop the LE duplicate of a dual-mode device (Classic is collected first,
    // and carries the actionable address).
    let mut seen = Vec::new();
    devices.retain(|d| {
        let key = d.name.to_lowercase();
        !seen.contains(&key) && {
            seen.push(key);
            true
        }
    });

    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    devices
}

#[derive(Clone, Copy)]
enum Transport {
    Classic,
    LowEnergy,
}

fn collect_paired(out: &mut Vec<Device>, transport: Transport) {
    let selector = match transport {
        Transport::Classic => BluetoothDevice::GetDeviceSelectorFromPairingState(true),
        Transport::LowEnergy => BluetoothLEDevice::GetDeviceSelectorFromPairingState(true),
    };
    let Ok(selector) = selector else { return };
    let Ok(collection) =
        DeviceInformation::FindAllAsyncAqsFilter(&selector).and_then(|op| op.get())
    else {
        return;
    };
    for info in collection {
        let Ok(name) = info.Name() else { continue };
        let name = name.to_string();
        if name.trim().is_empty() {
            continue;
        }
        let Ok(id) = info.Id() else { continue };
        let (connected, address) = device_facts(&id, transport);
        out.push(Device {
            id: address.map(|a| a.to_string()),
            name,
            connected,
        });
    }
}

/// Live `(connected, classic address)` for a device. The address is `Some` only
/// for classic devices, which are the ones `apply_item` can act on.
fn device_facts(id: &HSTRING, transport: Transport) -> (bool, Option<u64>) {
    match transport {
        Transport::Classic => {
            let Ok(dev) = BluetoothDevice::FromIdAsync(id).and_then(|op| op.get()) else {
                return (false, None);
            };
            let connected = dev.ConnectionStatus() == Ok(BluetoothConnectionStatus::Connected);
            (connected, dev.BluetoothAddress().ok())
        }
        Transport::LowEnergy => {
            let Ok(dev) = BluetoothLEDevice::FromIdAsync(id).and_then(|op| op.get()) else {
                return (false, None);
            };
            let connected = dev.ConnectionStatus() == Ok(BluetoothConnectionStatus::Connected);
            (connected, None)
        }
    }
}

/// Connect/disconnect a classic device via the Win32 service-state API. WinRT
/// has no connect/disconnect surface, so we drop to `BluetoothSetServiceState`,
/// which acts per installed service (BlueZ toggles the whole device in one call).
mod winbt {
    use std::mem::size_of;
    use windows::Win32::Devices::Bluetooth::{
        BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BLUETOOTH_FIND_RADIO_PARAMS,
        BLUETOOTH_SERVICE_DISABLE, BLUETOOTH_SERVICE_ENABLE, BluetoothEnumerateInstalledServices,
        BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindFirstRadio,
        BluetoothFindNextDevice, BluetoothFindNextRadio, BluetoothFindRadioClose,
        BluetoothSetServiceState,
    };
    use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE};
    use windows::core::GUID;

    /// Find the paired device by address, flip its connection, and report
    /// `(now_connected, name)`.
    pub fn toggle_connection(address: u64) -> Result<(bool, String), String> {
        let params = BLUETOOTH_FIND_RADIO_PARAMS {
            dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
        };
        let mut radio = HANDLE::default();
        let radio_find = match unsafe { BluetoothFindFirstRadio(&params, &mut radio) } {
            Ok(handle) => handle,
            Err(_) => return Err(super::NO_ADAPTER.to_string()),
        };

        let mut result = Err(super::GONE.to_string());
        loop {
            if let Some(info) = find_device(radio, address) {
                let connect = !info.fConnected.as_bool();
                let name = device_name(&info);
                result = set_services(radio, &info, connect).map(|()| (connect, name));
                unsafe { CloseHandle(radio) }.ok();
                break;
            }
            unsafe { CloseHandle(radio) }.ok();
            radio = HANDLE::default();
            if unsafe { BluetoothFindNextRadio(radio_find, &mut radio) }.is_err() {
                break;
            }
        }
        unsafe { BluetoothFindRadioClose(radio_find) }.ok();
        result
    }

    fn find_device(radio: HANDLE, address: u64) -> Option<BLUETOOTH_DEVICE_INFO> {
        let search = BLUETOOTH_DEVICE_SEARCH_PARAMS {
            dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
            fReturnAuthenticated: true.into(),
            fReturnRemembered: true.into(),
            fReturnUnknown: false.into(),
            fReturnConnected: true.into(),
            fIssueInquiry: false.into(),
            cTimeoutMultiplier: 0,
            hRadio: radio,
        };
        let mut info: BLUETOOTH_DEVICE_INFO = unsafe { std::mem::zeroed() };
        info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;

        let find = match unsafe { BluetoothFindFirstDevice(&search, &mut info) } {
            Ok(handle) => handle,
            Err(_) => return None,
        };
        let mut found = None;
        loop {
            if unsafe { info.Address.Anonymous.ullLong } == address {
                found = Some(info);
                break;
            }
            info = unsafe { std::mem::zeroed() };
            info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
            if unsafe { BluetoothFindNextDevice(find, &mut info) }.is_err() {
                break;
            }
        }
        unsafe { BluetoothFindDeviceClose(find) }.ok();
        found
    }

    fn device_name(info: &BLUETOOTH_DEVICE_INFO) -> String {
        let end = info
            .szName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szName.len());
        String::from_utf16_lossy(&info.szName[..end])
    }

    fn set_services(
        radio: HANDLE,
        info: &BLUETOOTH_DEVICE_INFO,
        connect: bool,
    ) -> Result<(), String> {
        // First pass: how many services the device has installed.
        let mut count: u32 = 0;
        unsafe { BluetoothEnumerateInstalledServices(Some(radio), info, &mut count, None) };
        if count == 0 {
            return Err("No connectable services on this device".to_string());
        }
        let mut guids = vec![GUID::default(); count as usize];
        let status = unsafe {
            BluetoothEnumerateInstalledServices(
                Some(radio),
                info,
                &mut count,
                Some(guids.as_mut_ptr()),
            )
        };
        if status != ERROR_SUCCESS.0 {
            return Err("Windows could not read the device services".to_string());
        }

        let flag = if connect {
            BLUETOOTH_SERVICE_ENABLE
        } else {
            BLUETOOTH_SERVICE_DISABLE
        };
        let mut any = false;
        for guid in &guids {
            if unsafe { BluetoothSetServiceState(Some(radio), info, guid, flag) } == ERROR_SUCCESS.0
            {
                any = true;
            }
        }
        if any {
            Ok(())
        } else {
            Err("Windows could not change the connection".to_string())
        }
    }
}
