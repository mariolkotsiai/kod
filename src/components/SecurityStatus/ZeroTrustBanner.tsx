interface SecurityControl {
  label: string;
  /** Must come from a real check (backend status), never a hardcoded true. */
  verified: boolean;
}

interface ZeroTrustBannerProps {
  error: string | null;
  /** Optional, backend-reported control status. Nothing security-related is claimed without it. */
  controls?: SecurityControl[];
}

export function ZeroTrustBanner({ error, controls }: ZeroTrustBannerProps) {
  return (
    <div className="space-y-2">
      {controls && controls.length > 0 && (
        <ul className="flex flex-wrap gap-2 text-xs">
          {controls.map((c) => (
            <li
              key={c.label}
              className={`rounded border px-3 py-1.5 ${
                c.verified
                  ? "border-emerald-800/70 bg-emerald-950/40 text-emerald-200"
                  : "border-amber-700 bg-amber-950/40 text-amber-200"
              }`}
            >
              {c.verified ? "Verified" : "Not verified"}: {c.label}
            </li>
          ))}
        </ul>
      )}
      {error && (
        <div className="rounded-lg border border-rose-700 bg-rose-950/60 px-4 py-2.5 text-xs text-rose-200">
          Error: {error}
        </div>
      )}
    </div>
  );
}
