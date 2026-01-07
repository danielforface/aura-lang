import { notFound } from "next/navigation";

import { docsNav } from "@/lib/docsNav";
import { getDoc } from "@/lib/docs";

export async function generateStaticParams() {
  // Catch-all route expects `slug` as string[].
  return docsNav.map((i) => ({ slug: i.slug.split("/") }));
}

export default async function DocPage({
  params,
}: {
  params: Promise<{ slug: string[] }>;
}) {
  const { slug } = await params;
  const joined = slug.join("/");

  const isKnown = docsNav.some((i) => i.slug === joined);
  if (!isKnown) return notFound();

  const doc = await getDoc(joined);

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
