# Aura website design system

The website has an independent product identity and avoids generic hardware/cyber visual language.

## Visual identity

- Base: warm paper / off-white.
- Typography: near-black editorial sans-serif with restrained mono for code/status.
- Primary language accent: indigo/violet.
- Proof state: green, used semantically rather than decoratively.
- Warning/evolving state: amber.
- Dark surfaces: reserved for source, solver and editor representations.

## Product hierarchy

The core visual sentence is:

```text
Source → Semantics → Proof → Execution → Feedback
```

Architecture diagrams, code panels and proof traces should reinforce that sentence.

## Avoid

- generic neon/cyber styling,
- hardware/silicon motifs,
- invented performance charts,
- simulated compiler results presented as live output,
- decorative badges that imply stronger evidence than exists,
- overly rounded SaaS-card layouts with no information hierarchy.

## Interaction

Interactive elements should explain the language or platform: pipeline exploration, proof semantics, docs navigation, syntax exploration. Motion is optional and must respect `prefers-reduced-motion`.
