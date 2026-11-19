export type Platform = "Android" | "Apple";
export type BackupMode = "Selective" | "FullData" | "FullImage";
export type RestoreMode = "Selective" | "FullData" | "FullImage";

export interface SlotStatus {
  slot: "Slot0" | "Slot1";
  occupied: boolean;
  device_id: string | null;
  platform: Platform | null;
}

export interface DiagnosticsReport {
  device_id: string;
  slot: "Slot0" | "Slot1";
  platform: Platform;
  thermals: { zone: string; celsius: number }[];
  battery: {
    health_percent: number;
    cycle_count: number;
    temperature_celsius: number;
  };
  storage: {
    total_bytes: number;
    used_bytes: number;
    system_bytes: number;
  };
  sensors: { sensor_name: string; operational: boolean; detail: string }[];
  digitizer: {
    rows: number;
    cols: number;
    dead_zones_detected: number;
    passed: boolean;
  };
}

export interface BackupResult {
  archive_path: string;
  sha256_hex: string;
  size_bytes: number;
  encrypted: boolean;
}