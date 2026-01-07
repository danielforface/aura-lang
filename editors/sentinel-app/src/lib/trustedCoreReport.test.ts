import { describe, expect, it } from "vitest";
import { buildTrustedCoreReport, renderTrustedCoreReportHtml } from "./trustedCoreReport";

describe("trusted core report", () => {
  it("renders HTML deterministically (snapshot)", () => {
    const report = buildTrustedCoreReport({
      generatedAt: "2026-01-01T00:00:00.000Z",
      uri: "file:///C:/tmp/main.aura",
      path: "C:\\tmp\\main.aura",
      projectRoot: "C:\\tmp",
      settings: { theme: "oneDark", formatOnSave: false, proofMode: "auto", proofDebounceMs: 450 },
      lspDiagnostics: [],
      proofsDiagnostics: [],
      mergedDiagnostics: [
        {
          range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
          severity: 1,
          source: "aura",
          code: "proof",
          message: "boom",
        },
      ],
      timelineDelta: { added: 1, removed: 0, changed: 0 },
    });

    const html = renderTrustedCoreReportHtml(report, (d) => `${d.source ?? ""} ${d.code ?? ""}`.trim());
    expect(html).toMatchSnapshot();
  });
});
