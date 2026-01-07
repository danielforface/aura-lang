import Link from "next/link";

const nav = [
  { href: "/docs/getting-started", label: "Docs" },
  { href: "/gallery", label: "Gallery" },
  { href: "/playground", label: "Playground" },
];

export function SiteHeader() {
  return (
    <header className="border-b border-black/10 bg-white/60 backdrop-blur dark:border-white/10 dark:bg-black/40">
      <div className="mx-auto flex h-14 w-full max-w-6xl items-center justify-between px-6">
        <Link href="/" className="font-semibold tracking-tight">
          Aura
        </Link>
        <nav className="flex items-center gap-4 text-sm text-zinc-700 dark:text-zinc-300">
          {nav.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className="rounded-full px-3 py-1 hover:bg-black/5 dark:hover:bg-white/10"
            >
              {item.label}
            </Link>
          ))}
        </nav>
      </div>
    </header>
  );
}
