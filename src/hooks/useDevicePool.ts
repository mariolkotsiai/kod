import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  BackupMode,
  BackupResult,
  DiagnosticsReport,
  RestoreMode,
  RestoreResult,
  SlotStatus,
} from "../types/toolkit";

/** The backend rejects with a plain string; fall back gracefully for any other shape. */
function toMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  try {
    return JSON.stringify(err);
  } catch {
    return "Unknown error";
  }
}

export function useDevicePool() {
  const [slots, setSlots] = useState<SlotStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshSlots = useCallback(async () => {
    try {
      const data = await invoke<SlotStatus[]>("get_slots");
      setSlots(data);
      setError(null);
    } catch (err) {
      setError(toMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshSlots();
  }, [refreshSlots]);

  const runDiagnostics = useCallback(async (slotIdx: 0 | 1): Promise<DiagnosticsReport | null> => {
    setBusy(true);
    try {
      const report = await invoke<DiagnosticsReport>("run_diagnostics", { slotIdx });
      setError(null);
      return report;
    } catch (err) {
      setError(toMessage(err));
      return null;
    } finally {
      setBusy(false);
    }
  }, []);

  const runBackup = useCallback(
    async (slotIdx: 0 | 1, mode: BackupMode, passphrase: string): Promise<BackupResult | null> => {
      setBusy(true);
      try {
        const res = await invoke<BackupResult>("run_backup", { slotIdx, mode, passphrase });
        setError(null);
        return res;
      } catch (err) {
        setError(toMessage(err));
        return null;
      } finally {
        setBusy(false);
      }
    },
    []
  );

  const runRestore = useCallback(
    async (
      slotIdx: 0 | 1,
      backupPath: string,
      mode: RestoreMode,
      passphrase: string,
      expectedSha256?: string
    ): Promise<RestoreResult | null> => {
      setBusy(true);
      try {
        const res = await invoke<RestoreResult>("run_restore", {
          slotIdx,
          backupPath,
          mode,
          passphrase,
          expectedSha256: expectedSha256 ?? null,
        });
        setError(null);
        return res;
      } catch (err) {
        setError(toMessage(err));
        return null;
      } finally {
        setBusy(false);
      }
    },
    []
  );

  return { slots, error, busy, refreshSlots, runDiagnostics, runBackup, runRestore };
}
