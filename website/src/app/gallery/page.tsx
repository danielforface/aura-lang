import Link from "next/link";
import { Icon } from "@/components/Icon";
import { SectionHeading } from "@/components/SectionHeading";

export const metadata = { title: "Examples", description: "Repository-backed Aura examples across verification, Lumina, networking, IoT and AI integration." };

const examples=[
 {title:"Verification features",meta:"contracts · invariants · decreases · quantifiers",body:"A small repository example that exercises verifier-facing syntax and notes that quantifiers require the thorough SMT profile.",href:"https://github.com/danielforface/aura-lang/blob/main/examples/verification/verify_features.aura"},
 {title:"Lumina grid + media",meta:"grid · Box · Button · async callbacks",body:"The grid_image_audio example demonstrates the evolving application layer with layout, image/media-oriented work and callback flow.",href:"https://github.com/danielforface/aura-lang/blob/main/examples/grid_image_audio.aura"},
 {title:"Move-oriented safety",meta:"resource semantics",body:"The aura-move example tree exists as an integration surface for resource/move-oriented language work.",href:"https://github.com/danielforface/aura-lang/tree/main/examples/aura-move"},
 {title:"IoT safety",meta:"Nexus / domain plugin",body:"An example vertical slice for safety-oriented IoT integration. Its existence demonstrates integration work, not a universal IoT framework guarantee.",href:"https://github.com/danielforface/aura-lang/tree/main/examples/aura-iot-safe"},
 {title:"Vision / ONNX",meta:"AI plugin · native bridge",body:"Aura Vision exercises an ONNX-oriented integration path and shape-aware language/plugin direction. Claims remain tied to the example and implementation surface.",href:"https://github.com/danielforface/aura-lang/tree/main/examples/aura-vision-safe"},
 {title:"TCP echo server",meta:"networking surface",body:"A compact example showing the repository’s networking-facing language/stdlib direction.",href:"https://github.com/danielforface/aura-lang/blob/main/examples/tcp_echo_server.aura"},
] as const;
export default function GalleryPage(){return <>
<section className="subpage-hero"><div className="page-shell subpage-hero-grid"><div><div className="eyebrow">Examples</div><h1>Use vertical slices to understand the platform.</h1><p>Aura’s examples reveal which pieces of the language, verifier, plugins, bridges and application stack have actually been assembled. The gallery deliberately avoids turning the existence of an example into a blanket stability claim.</p></div><aside className="subpage-summary"><dl><div><dt>Verification</dt><dd>repository fixture</dd></div><div><dt>Lumina</dt><dd>application examples</dd></div><div><dt>AI / IoT</dt><dd>domain integrations</dd></div><div><dt>Source</dt><dd>inspectable</dd></div></dl></aside></div></section>
<section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Repository examples" title="Code first, claims second."/><div className="gallery-grid">{examples.map(ex=><a className="gallery-card" href={ex.href} target="_blank" rel="noreferrer" key={ex.title}><div className="gallery-visual"><span>{ex.title.slice(0,2).toUpperCase()}</span></div><div className="gallery-copy"><span className="gallery-meta">{ex.meta}</span><h3>{ex.title}</h3><p>{ex.body}</p><span className="button-text">View source <Icon name="external" size={13}/></span></div></a>)}</div></div></section>
<section className="page-section compact"><div className="page-shell"><div className="closing-panel"><div><h2>Examples are evidence of integration — not substitutes for a specification.</h2><p>For stable semantics, follow the current implementation and compact language reference.</p></div><Link href="/language" className="button-secondary">Language overview</Link></div></div></section>
</>}
