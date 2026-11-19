import type { SlotStatus } from "../../types/toolkit";

interface SlotCardProps {
  slot: SlotStatus;
  selected: boolean;
  onSelect: () => void;
}

export function SlotCard({ slot, selected, onSelect }: SlotCardProps) {
  const label = slot.slot === "Slot0" ? "DeviceID 0" : "DeviceID 1";
  return (
    <button
      type="button"
      onClick={onSelect}
      className={`w-full text-left rounded-lg border p-4 transition ${
        selected
          ? "border-cyan-400 bg-slate-800/90"
          : "border-slate-700 bg-slate-900/70 hover:border-slate-500"
      }`}
    >
      <div className="flex items-center justify-between">
        <span className="font-mono text-sm font-semibold text-cyan-300">{label}</span>
        <span
          className={`rounded px-2 py-0.5 text-xs font-medium ${
            slot.occupied ? "bg-emerald-950 text-emerald-300" : "bg-slate-800 text-slate-400"
          }`}
        >
          {slot.occupied ? slot.platform ?? "ACTIVE" : "EMPTY"}
        </span>
      </div>
      <p className="mt-2 truncate font-mono text-xs text-slate-300">
        {slot.occupied ? slot.device_id ?? "Connected" : "No USB device bound"}
      </p>
    </button>
  );
}