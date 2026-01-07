import { describe, expect, it } from "vitest";
import { diagStableId, mergeDiagnostics } from "./diagnostics";

describe("diagnostics", () => {
  it("diagStableId is deterministic", () => {
    const d = {
      range: { start: { line: 1, character: 2 }, end: { line: 1, character: 3 } },
      severity: 1,
      code: "X",
      source: "aura",
      message: "boom",
    };
    expect(diagStableId(d)).toBe("1:2-1:3|1|X|aura|boom");
  });

  it("mergeDiagnostics prefers more severe when IDs collide", () => {
    const a = {
      id: "same",
      range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
      severity: 2,
      message: "warn",
    };
    const b = { ...a, severity: 1, message: "err" };

    const merged = mergeDiagnostics([a], [b]);
    expect(merged).toHaveLength(1);
    expect(merged[0].severity).toBe(1);
    expect(merged[0].message).toBe("err");
  });
});
