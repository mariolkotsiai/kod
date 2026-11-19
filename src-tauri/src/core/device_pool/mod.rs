use crate::core::pal::{DeviceSlot, MobileDevice, PalError, Platform};
use rusb::{Device, UsbContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

// Known USB Vendor IDs for Android (Google/AOSP/OEMs) and Apple devices.
const APPLE_VID: u16 = 0x05AC;
const GOOGLE_VID: u16 = 0x18D1;
const SAMSUNG_VID: u16 = 0x04E8;
const XIAOMI_VID: u16 = 0x2717;
const ONEPLUS_VID: u16 = 0x22D9;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredUsbDevice {
    pub serial_or_bus_key: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub platform: Platform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotStatus {
    pub slot: DeviceSlot,
    pub occupied: bool,
    pub device_id: Option<String>,
    pub platform: Option<Platform>,
}

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("All device slots (0 and 1) are currently occupied")]
    PoolFull,
    #[error("Device '{0}' is already registered in an active slot")]
    DuplicateDevice(String),
    #[error("Slot {0:?} is currently empty")]
    EmptySlot(DeviceSlot),
    #[error("USB enumeration error: {0}")]
    UsbError(String),
    #[error("PAL error: {0}")]
    Pal(#[from] PalError),
}

pub struct DevicePool {
    slots: RwLock<HashMap<u8, Arc<dyn MobileDevice>>>,
}

impl Default for DevicePool {
    fn default() -> Self {
        Self::new()
    }
}

impl DevicePool {
    pub fn new() -> Self {
        Self {
            slots: RwLock::new(HashMap::with_capacity(2)),
        }
    }

    /// Scans USB buses via rusb and classifies recognized Android & Apple mobile endpoints.
    pub fn scan_usb_devices(&self) -> Result<Vec<DiscoveredUsbDevice>, PoolError> {
        let context = rusb::Context::new().map_err(|e| PoolError::UsbError(e.to_string()))?;
        let devices = context
            .devices()
            .map_err(|e| PoolError::UsbError(e.to_string()))?;

        let mut discovered = Vec::new();
        for device in devices.iter() {
            if let Some(item) = Self::classify_usb_device(&device) {
                discovered.push(item);
            }
        }
        Ok(discovered)
    }

    fn classify_usb_device<T: UsbContext>(device: &Device<T>) -> Option<DiscoveredUsbDevice> {
        let desc = device.device_descriptor().ok()?;
        let vid = desc.vendor_id();
        let pid = desc.product_id();

        let platform = match vid {
            APPLE_VID => Platform::Apple,
            GOOGLE_VID | SAMSUNG_VID | XIAOMI_VID | ONEPLUS_VID => Platform::Android,
            _ => return None,
        };

        let key = format!(
            "bus{:03}-addr{:03}-{:04x}:{:04x}",
            device.bus_number(),
            device.address(),
            vid,
            pid
        );

        Some(DiscoveredUsbDevice {
            serial_or_bus_key: key,
            vendor_id: vid,
            product_id: pid,
            platform,
        })
    }

    /// Attaches a MobileDevice into the first available isolated slot (Slot0 or Slot1).
    pub async fn attach_device(&self, device: Arc<dyn MobileDevice>) -> Result<DeviceSlot, PoolError> {
        let mut guard = self.slots.write().await;
        let new_id = device.device_id().to_string();

        for existing in guard.values() {
            if existing.device_id() == new_id {
                return Err(PoolError::DuplicateDevice(new_id));
            }
        }

        let assigned_slot = if !guard.contains_key(&(DeviceSlot::Slot0 as u8)) {
            DeviceSlot::Slot0
        } else if !guard.contains_key(&(DeviceSlot::Slot1 as u8)) {
            DeviceSlot::Slot1
        } else {
            return Err(PoolError::PoolFull);
        };

        guard.insert(assigned_slot as u8, device);
        Ok(assigned_slot)
    }

    /// Detaches and returns the device bound to a specific slot upon USB removal or session close.
    pub async fn detach_slot(&self, slot: DeviceSlot) -> Result<Arc<dyn MobileDevice>, PoolError> {
        let mut guard = self.slots.write().await;
        guard
            .remove(&(slot as u8))
            .ok_or(PoolError::EmptySlot(slot))
    }

    /// Retrieves a clone of the Arc handle for a given slot without holding the pool lock across I/O.
    pub async fn get_device(&self, slot: DeviceSlot) -> Result<Arc<dyn MobileDevice>, PoolError> {
        let guard = self.slots.read().await;
        guard
            .get(&(slot as u8))
            .cloned()
            .ok_or(PoolError::EmptySlot(slot))
    }

    /// Returns snapshot status for both DeviceID 0 and DeviceID 1 slots.
    pub async fn list_slots(&self) -> Vec<SlotStatus> {
        let guard = self.slots.read().await;
        [DeviceSlot::Slot0, DeviceSlot::Slot1]
            .into_iter()
            .map(|slot| match guard.get(&(slot as u8)) {
                Some(dev) => SlotStatus {
                    slot,
                    occupied: true,
                    device_id: Some(dev.device_id().to_string()),
                    platform: Some(dev.platform()),
                },
                None => SlotStatus {
                    slot,
                    occupied: false,
                    device_id: None,
                    platform: None,
                },
            })
            .collect()
    }
}