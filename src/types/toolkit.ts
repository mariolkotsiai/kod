export type Platform = "Android" | "Apple";
export type BackupMode = "Selective" | "FullData" | "FullImage";
export type RestoreMode = "Selective" | "FullData" | "FullImage";
export type SlotName = "Slot0" | "Slot1";

export interface SlotStatus {
  slot: SlotName;
  occupied: boolean;
  device_id: string | null;
  platform: Platform | null;
}

/** Every field is nullable: null means "not measured on this device", never a made-up value. */
export interface BatteryTelemetry {
  level_percent: number | null;
  health_percent: number | null;
  cycle_count: number | null;
  temperature_celsius: number | null;
}

export interface StorageBreakdown {
  total_bytes: number;
  used_bytes: number;
  system_bytes: number | null;
}

export interface DigitizerMatrixResult {
  rows: number;
  cols: number;
  dead_zones_detected: number;
  passed: boolean;
}

export interface DiagnosticsReport {
  device_id: string;
  slot: SlotName;
  platform: Platform;
  thermals: { zone: string; celsius: number }[];
  battery: BatteryTelemetry;
  /** null = not measured */
  storage: StorageBreakdown | null;
  sensors: { sensor_name: string; operational: boolean; detail: string }[];
  /** null = not measured */
  digitizer: DigitizerMatrixResult | null;
}

export interface BackupResult {
  archive_path: string;
  sha256_hex: string;
  size_bytes: number;
  encrypted: boolean;
}

export interface RestoreResult {
  verified_sha256: string;
  restored_items: number;
}
