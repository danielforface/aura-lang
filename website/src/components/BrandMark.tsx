import Link from "next/link";

export function AuraGlyph({ className = "" }: { className?: string }) {
  return (
    <svg
      aria-hidden="true"
      className={className}
      viewBox="0 0 40 40"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path d="M20 4.5 34 34H27.2L23.9 26.2H15.9L12.7 34H6L20 4.5Z" fill="currentColor" />
      <path d="M18.1 20.7h3.8L20 15.9l-1.9 4.8Z" fill="var(--paper)" />
      <circle cx="31.8" cy="8.2" r="3.2" fill="var(--proof)" />
    </svg>
  );
}

export function BrandMark({ compact = false }: { compact?: boolean }) {
  return (
    <Link href="/" className="brand-mark" aria-label="Aura home">
      <AuraGlyph className="brand-glyph" />
      {!compact ? (
        <span className="brand-type">
          <strong>Aura</strong>
          <span>2026 Edition</span>
        </span>
      ) : null}
    </Link>
  );
}
