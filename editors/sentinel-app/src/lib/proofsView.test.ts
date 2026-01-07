import { describe, expect, it } from "vitest";
import { renderStructuredProofs } from "./proofsView";

describe("proofs view", () => {
  it("renders grouped proofs (snapshot)", () => {
    const doc = {
      outlineItems: [
        {
          name: "foo",
          range: { start: { line: 0, character: 0 }, end: { line: 10, character: 0 } },
          selection_range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } },
          children: [],
        },
      ],
      mergedDiagnostics: [
        {
          range: { start: { line: 1, character: 0 }, end: { line: 1, character: 1 } },
          severity: 1,
          source: "aura",
          code: "proof",
          message: "failed obligation",
        },
      ],
    };

    const html = renderStructuredProofs(doc, { selectedProofId: undefined, lastProofDelta: { added: 1, removed: 0, changed: 0 } });
    expect(html).toMatchSnapshot();
  });
});
