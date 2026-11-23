import { useState } from "react";
import { BackupPanel } from "./components/BackupRestore/BackupPanel";
import { SlotCard } from "./components/DeviceManager/SlotCard";
import { DiagnosticsPanel } from "./components/Diagnostics/DiagnosticsPanel";
import { ZeroTrustBanner } from "./components/SecurityStatus/ZeroTrustBanner";
import { useDevicePool } from "./hooks/useDevicePool";
import type { DiagnosticsReport } from "./types/toolkit";

// Minimal host shell so the project builds. The real UI work is Phase 3.
export default function App() {
  const { slots, error, busy, refreshSlots, runDiagnostics, runBackup } = useDevicePool();
  const [selected, setSelected] = useState<0 | 1>(0);
  const [report, setReport] = useState<DiagnosticsReport | null>(null);

  const hasDevice = slots[selected]?.occupied ?? false;

  const handleRunDiagnostics = async () => {
    setReport(await runDiagnostics(selected));
  };

  return (
    <main className="min-h-screen bg-slate-950 p-6 text-slate-100">
      <div className="mx-auto max-w-4xl space-y-4">
        <header className="flex items-center justify-between">
          <h1 className="text-lg font-semibold">OmniMobile Toolkit</h1>
          <button
            type="button"
            onClick={() => void refreshSlots()}
            className="rounded border border-slate-700 px-3 py-1.5 text-xs hover:border-slate-500"
          >
            Refresh
          </button>
        </header>

        <ZeroTrustBanner error={error} />

        <div className="grid gap-3 sm:grid-cols-2">
          {slots.map((slot, idx) => (
            <SlotCard
              key={slot.slot}
              slot={slot}
              selected={selected === idx}
              onSelect={() => {
                setSelected(idx === 0 ? 0 : 1);
                setReport(null);
              }}
            />
          ))}
        </div>

        <DiagnosticsPanel
          report={report}
          disabled={busy || !hasDevice}
          onRun={() => void handleRunDiagnostics()}
        />
        <BackupPanel
          disabled={busy || !hasDevice}
          onBackup={(mode, passphrase) => runBackup(selected, mode, passphrase)}
        />
      </div>
    </main>
  );
}
