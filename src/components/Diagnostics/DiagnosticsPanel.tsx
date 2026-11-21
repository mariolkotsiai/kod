import type { DiagnosticsReport } from "../../types/toolkit";
import { formatBytes } from "../../utils/format";

interface DiagnosticsPanelProps {
  report: DiagnosticsReport | null;
  onRun: () => void;
  disabled: boolean;
}

/** null means "not measured": show that explicitly instead of a default value. */
function orNA(value: number | null, suffix = ""): string {
  return value === null ? "n/a" : `${value}${suffix}`;
}

export function DiagnosticsPanel({ report, onRun, disabled }: DiagnosticsPanelProps) {
  return (
    <div className="rounded-lg border border-slate-700 bg-slate-900/80 p-4 text-slate-100">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-cyan-300">
          Hardware & Digitizer Diagnostics
        </h2>
        <button
          type="button"
          disabled={disabled}
          onClick={onRun}
          className="rounded bg-cyan-500 px-3 py-1.5 text-xs font-semibold text-slate-950 disabled:opacity-40"
        >
          Run Diagnostics
        </button>
      </div>

      {report ? (
        <div className="mt-4 grid grid-cols-1 gap-3 text-xs sm:grid-cols-3">
          <div className="rounded border border-slate-800 bg-slate-950/70 p-3">
            <div className="text-slate-400">Battery</div>
            <div className="mt-1 space-y-0.5 font-mono text-sm text-emerald-300">
              <div>Level: {orNA(report.battery.level_percent, "%")}</div>
              <div>Health: {orNA(report.battery.health_percent, "%")}</div>
              <div>Cycles: {orNA(report.battery.cycle_count)}</div>
              <div>Temp: {orNA(report.battery.temperature_celsius, "°C")}</div>
            </div>
          </div>
          <div className="rounded border border-slate-800 bg-slate-950/70 p-3">
            <div className="text-slate-400">Storage</div>
            <div className="mt-1 font-mono text-sm text-slate-200">
              {report.storage
                ? `${formatBytes(report.storage.used_bytes)} / ${formatBytes(report.storage.total_bytes)}`
                : "Not measured"}
            </div>
          </div>
          <div className="rounded border border-slate-800 bg-slate-950/70 p-3">
            <div className="text-slate-400">Digitizer Matrix</div>
            <div className="mt-1 font-mono text-sm text-cyan-200">
              {report.digitizer
                ? `${report.digitizer.rows}x${report.digitizer.cols} | ${
                    report.digitizer.passed ? "PASS" : "FAIL"
                  }`
                : "Not measured"}
            </div>
          </div>
        </div>
      ) : (
        <p className="mt-3 text-xs text-slate-400">No diagnostics report collected for active slot.</p>
      )}
    </div>
  );
}
