import Link from "next/link";

import { docsNav } from "@/lib/docsNav";

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-1 gap-8 lg:grid-cols-[260px,1fr]">
      <aside className="rounded-2xl border border-black/10 bg-white/60 p-5 dark:border-white/10 dark:bg-black/40">
        <div className="text-sm font-semibold tracking-tight">Aura Book</div>
        <nav className="mt-4 flex flex-col gap-1 text-sm text-zinc-700 dark:text-zinc-300">
          {docsNav.map((item) => (
            <Link
              key={item.slug}
              href={`/docs/${item.slug}`}
              className="rounded-lg px-3 py-2 hover:bg-black/5 dark:hover:bg-white/10"
            >
              {item.title}
            </Link>
          ))}
        </nav>
      </aside>
      <section className="rounded-2xl border border-black/10 bg-white/60 p-6 dark:border-white/10 dark:bg-black/40">
        <article className="prose prose-zinc max-w-none dark:prose-invert">
          {children}
        </article>
      </section>
    </div>
  );
}
