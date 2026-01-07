import { describe, expect, it } from "vitest";
import { linkifyConsoleLine } from "./consoleLinkify";

describe("console linkify", () => {
  it("linkifies a plain Windows path", () => {
    const s = "C:\\src\\main.aura:12:34 something";
    const html = linkifyConsoleLine(s);
    expect(html).toContain("class=\"consoleLink\"");
    expect(html).toContain("data-path=\"C%3A%5Csrc%5Cmain.aura\"");
    expect(html).toContain("data-line=\"12\"");
    expect(html).toContain("data-col=\"34\"");
  });

  it("linkifies a bracketed path", () => {
    const s = "[C:\\src\\main.aura:1:2]";
    const html = linkifyConsoleLine(s);
    expect(html).toContain("[");
    expect(html).toContain("]");
    expect(html).toContain("C:\\src\\main.aura:1:2");
  });

  it("escapes HTML outside links", () => {
    const s = "<tag> C:\\x\\y.aura:1:1";
    const html = linkifyConsoleLine(s);
    expect(html).toContain("&lt;tag&gt;");
  });
});
