import { useState } from "react";
import type { BackupMode, BackupResult } from "../../types/toolkit";

const MIN_PASSPHRASE_CHARS = 12;

/** Modes the backend implements today; the others are listed but disabled. */
const MODES: { value: BackupMode; label: string; available: boolean }[] = [
  { value: "Selective", label: "Selective", available: true },
  { value: "FullData", label: "FullData (not implemented)", available: false },
  { value: "FullImage", label: "FullImage (not implemented)", available: false },
];

interface BackupPanelProps {
  disabled: boolean;
  onBackup: (mode: BackupMode, passphrase: string) => Promise<BackupResult | null>;
}

export function BackupPanel({ disabled, onBackup }: BackupPanelProps) {
  const [mode, setMode] = useState<BackupMode>("Selective");
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [lastBackup, setLastBackup] = useState<BackupResult | null>(null);

  // Count code points like the backend (chars().count()), not UTF-16 units.
  const longEnough = [...passphrase].length >= MIN_PASSPHRASE_CHARS;
  const matches = passphrase === confirm;
  const canSubmit = !disabled && longEnough && matches;

  const handleTrigger = async () => {
    if (!canSubmit) return;
    try {
      const result = await onBackup(mode, passphrase);
      if (result) setLastBackup(result);
    } finally {
      // Never keep the secret in component state, whether the backup succeeded or failed.
      setPassphrase("");
      setConfirm("");
    }
  };

  const inputClass =
    "flex-1 rounded border border-slate-700 bg-slate-950 px-3 py-2 text-xs text-slate-100";

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
          {MODES.map((m) => (
            <option key={m.value} value={m.value} disabled={!m.available}>
              {m.label}
            </option>
          ))}
        </select>
        <input
          type="password"
          autoComplete="new-password"
          placeholder={`Passphrase (min ${MIN_PASSPHRASE_CHARS} chars)`}
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
          className={inputClass}
        />
        <input
          type="password"
          autoComplete="new-password"
          placeholder="Confirm passphrase"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          className={inputClass}
        />
        <button
          type="button"
          disabled={!canSubmit}
          onClick={handleTrigger}
          className="rounded bg-emerald-500 px-4 py-2 text-xs font-semibold text-slate-950 disabled:opacity-40"
        >
          Create Backup
        </button>
      </div>
      {confirm.length > 0 && !matches && (
        <p className="mt-2 text-xs text-rose-300">Passphrases do not match.</p>
      )}
      <p className="mt-2 text-xs text-slate-400">
        A lost passphrase cannot be recovered: the backup cannot be decrypted without it.
      </p>
      {lastBackup && (
        <p className="mt-3 break-all font-mono text-xs text-emerald-300">
          SHA-256: {lastBackup.sha256_hex} ({lastBackup.size_bytes} B)
        </p>
      )}
    </div>
  );
}
