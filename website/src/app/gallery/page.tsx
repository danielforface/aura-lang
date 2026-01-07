import Link from "next/link";

const projects = [
  {
    slug: "kinetic-engine",
    title: "Kinetic Engine",
    description: "Raylib demo showcasing real-time visuals via the Universal Bridge.",
  },
  {
    slug: "aura-vision",
    title: "Aura Vision",
    description: "ONNX demo showcasing shape-safe inference with Z3 verification.",
  },
];

export default function GalleryPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-3xl font-semibold tracking-tight">Project Gallery</h1>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {projects.map((p) => (
          <Link
            key={p.slug}
            href={`/gallery/${p.slug}`}
            className="rounded-2xl border border-black/10 bg-white/60 p-6 hover:bg-white/80 dark:border-white/10 dark:bg-black/40 dark:hover:bg-black/60"
          >
            <div className="text-lg font-semibold">{p.title}</div>
            <p className="mt-2 text-sm leading-6 text-zinc-700 dark:text-zinc-300">
              {p.description}
            </p>
          </Link>
        ))}
      </div>
    </div>
  );
}
