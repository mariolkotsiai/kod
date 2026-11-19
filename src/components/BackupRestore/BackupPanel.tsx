import { useState } from "react";
import type { BackupMode, BackupResult } from "../../types/toolkit";

interface BackupPanelProps {
  disabled: boolean;
  onBackup: (mode: BackupMode, passphrase: string) => Promise<BackupResult | null>;
}

export function BackupPanel({ disabled, onBackup }: BackupPanelProps) {
  const [mode, setMode] = useState<BackupMode>("Selective");
  const [passphrase, setPassphrase] = useState("");
  const [lastBackup, setLastBackup] = useState<BackupResult | null>(null);

  const handleTrigger = async () => {
    if (passphrase.length < 12) return;
    const result = await onBackup(mode, passphrase);
    if (result) {
      setLastBackup(result);
      setPassphrase("");
    }
  };

  return (
    <div className="rounded-lg border border-slate-700 bg-slate-900/80 p-4 text-slate-100">
      <h2 className="text-sm font-semibold uppercase tracking-wider text-cyan-300">
        Encrypted Backup (Argon2id + AES-256-GCM)
      </h2>
      <div className="mt-3 flex flex-col gap-2 sm:flex-row">
        <select
          value={mode}
          onChange={(e) => setMode(e.target.value as BackupMode)}
          className="rounded border border-slate-700 bg-slate-950 px-3 py-2 text-xs text-slate-100"
        >
          <option value="Selective">Selective</option>
          <option value="FullData">FullData</option>
          <option value="FullImage">FullImage</option>
        </select>
        <input
          type="password"
          placeholder="Passphrase (min 12 chars)"
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
          className="flex-1 rounded border border-slate-700 bg-slate-950 px-3 py-2 text-xs text-slate-100"
        />
        <button
          type="button"
          disabled={disabled || passphrase.length < 12}
          onClick={handleTrigger}
          className="rounded bg-emerald-500 px-4 py-2 text-xs font-semibold text-slate-950 disabled:opacity-40"
        >
          Create Backup
        </button>
      </div>
      {lastBackup && (
        <p className="mt-3 break-all font-mono text-xs text-emerald-300">
          SHA-256: {lastBackup.sha256_hex} ({lastBackup.size_bytes} B)
        </p>
      )}
    </div>
  );
}