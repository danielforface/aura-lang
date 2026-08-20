import type { MetadataRoute } from "next";
export const dynamic = "force-static";

export default function manifest():MetadataRoute.Manifest{return {name:"Aura Programming Language",short_name:"Aura",description:"Proof-driven systems programming language and developer platform.",start_url:"/",display:"standalone",background_color:"#f5f3ee",theme_color:"#5d4ff2"}}
