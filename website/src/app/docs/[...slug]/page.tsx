import { notFound } from "next/navigation";
import { docsNav } from "@/lib/docsNav";
import { getDoc } from "@/lib/docs";

export async function generateStaticParams(){return docsNav.filter(i=>i.slug.includes("/")).map(i=>({slug:i.slug.split("/")}))}
export async function generateMetadata({params}:{params:Promise<{slug:string[]}>}){const {slug}=await params;const joined=slug.join("/");if(!docsNav.some(i=>i.slug===joined)) return {};const doc=await getDoc(joined);return {title:doc.frontmatter.title,description:doc.frontmatter.description}}
export default async function DocPage({params}:{params:Promise<{slug:string[]}>}){const {slug}=await params;const joined=slug.join("/");if(!docsNav.some(i=>i.slug===joined)) return notFound();const doc=await getDoc(joined);return <><h1>{doc.frontmatter.title}</h1>{doc.frontmatter.description?<p className="lead">{doc.frontmatter.description}</p>:null}{doc.content}</>}
