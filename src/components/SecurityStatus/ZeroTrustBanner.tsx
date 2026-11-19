interface ZeroTrustBannerProps {
  error: string | null;
}

export function ZeroTrustBanner({ error }: ZeroTrustBannerProps) {
  return (
    <div className="space-y-2">
      <div className="rounded-lg border border-emerald-800/70 bg-emerald-950/40 px-4 py-2.5 text-xs text-emerald-200">
        Zero-Trust Active: Argon2id + AES-256-GCM • ZeroizeOnDrop • No-Shell Subprocess Execution
      </div>
      {error && (
        <div className="rounded-lg border border-rose-700 bg-rose-950/60 px-4 py-2.5 text-xs text-rose-200">
          Error: {error}
        </div>
      )}
    </div>
  );
}