import type { MetadataRoute } from "next";
import { DOC_GROUPS } from "@/lib/siteData";
export const dynamic = "force-static";

const base="https://aura.geniuses.team";
export default function sitemap():MetadataRoute.Sitemap{
  const pages=["","/language","/verification","/toolchain","/ecosystem","/status","/downloads","/releases","/gallery","/playground"];
  const docs=DOC_GROUPS.flatMap(g=>g.items.map(([slug])=>`/docs/${slug}`));
  return [...pages,...docs].map(path=>({url:`${base}${path}`,changeFrequency:"weekly",priority:path === "" ? 1 : path.startsWith("/docs/") ? 0.72 : 0.82}));
}
