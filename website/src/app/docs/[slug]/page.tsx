import { notFound } from "next/navigation";

import { docsNav } from "@/lib/docsNav";
import { getDoc } from "@/lib/docs";

export async function generateStaticParams() {
  return docsNav
    .filter((i) => !i.slug.includes("/"))
    .map((i) => ({ slug: i.slug }));
}

export default async function DocPage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;

  const isKnown = docsNav.some((i) => i.slug === slug);
  if (!isKnown) return notFound();

  const doc = await getDoc(slug);

  return (
    <>
      <h1 className="mt-0">{doc.frontmatter.title}</h1>
      {doc.frontmatter.description ? (
        <p className="lead">{doc.frontmatter.description}</p>
      ) : null}
      {doc.content}
    </>
  );
}
