import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { BackupMode, BackupResult, DiagnosticsReport, SlotStatus } from "../types/toolkit";

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
      setError(err instanceof Error ? err.message : String(err));
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
      setError(err instanceof Error ? err.message : String(err));
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
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setBusy(false);
      }
    },
    []
  );

  return { slots, error, busy, refreshSlots, runDiagnostics, runBackup };
}