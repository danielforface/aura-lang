import { escapeHtml } from "./html";

export function linkifyConsoleLine(line: string): string {
  // Supports:
  // - Windows paths: C:\dir\file.aura:12:34
  // - Bracketed forms: [C:\dir\file.aura:12:34]
  // - POSIX paths: /dir/file.aura:12:34
  const re = /\[((?:[A-Za-z]:\\|\/)[^\]\r\n]+?):(\d+):(\d+)\]|(^|\s)((?:[A-Za-z]:\\|\/)[^\s:]+?):(\d+):(\d+)(?=$|\s)/g;

  let out = "";
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(line)) !== null) {
    out += escapeHtml(line.slice(last, m.index));

    if (m[1]) {
      const filePath = m[1];
      const lineStr = m[2];
      const colStr = m[3];
      const label = `${filePath}:${lineStr}:${colStr}`;
      const link = `<a class="consoleLink" data-path="${encodeURIComponent(filePath)}" data-line="${encodeURIComponent(
        String(lineStr)
      )}" data-col="${encodeURIComponent(String(colStr))}">${escapeHtml(label)}</a>`;
      out += `[${link}]`;
    } else {
      const prefix = m[4] ?? "";
      const filePath = m[5];
      const lineStr = m[6];
      const colStr = m[7];
      out += escapeHtml(prefix);
      const label = `${filePath}:${lineStr}:${colStr}`;
      const link = `<a class="consoleLink" data-path="${encodeURIComponent(filePath)}" data-line="${encodeURIComponent(
        String(lineStr)
      )}" data-col="${encodeURIComponent(String(colStr))}">${escapeHtml(label)}</a>`;
      out += link;
    }

    last = m.index + m[0].length;
  }

  out += escapeHtml(line.slice(last));
  return out;
}
