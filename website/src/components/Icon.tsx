export type IconName =
  | "arrow"
  | "check"
  | "code"
  | "cube"
  | "external"
  | "github"
  | "layers"
  | "menu"
  | "package"
  | "proof"
  | "shield"
  | "terminal"
  | "x";

export function Icon({ name, size = 18, className = "" }: { name: IconName; size?: number; className?: string }) {
  const common = {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.7,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    className,
    "aria-hidden": true,
  };

  switch (name) {
    case "arrow":
      return <svg {...common}><path d="M5 12h14M14 7l5 5-5 5" /></svg>;
    case "check":
      return <svg {...common}><path d="m5 12 4 4L19 6" /></svg>;
    case "code":
      return <svg {...common}><path d="m9 18-6-6 6-6M15 6l6 6-6 6M14 3l-4 18" /></svg>;
    case "cube":
      return <svg {...common}><path d="m12 2 8 4.5v11L12 22l-8-4.5v-11L12 2Z" /><path d="m4.5 6.8 7.5 4.3 7.5-4.3M12 11v10" /></svg>;
    case "external":
      return <svg {...common}><path d="M14 5h5v5M19 5l-8 8" /><path d="M18 13v5a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" /></svg>;
    case "github":
      return <svg {...common}><path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3.3-.4 6.8-1.6 6.8-7A5.4 5.4 0 0 0 19.3 4 5 5 0 0 0 19.1.6S17.9.2 15 2a13.4 13.4 0 0 0-7 0C5.1.2 3.9.6 3.9.6A5 5 0 0 0 3.7 4a5.4 5.4 0 0 0-1.5 3.7c0 5.4 3.5 6.6 6.8 7A4.8 4.8 0 0 0 8 18v4" /><path d="M8 19c-3 .9-3-1.5-4-2" /></svg>;
    case "layers":
      return <svg {...common}><path d="m12 2 9 5-9 5-9-5 9-5Z" /><path d="m3 12 9 5 9-5M3 17l9 5 9-5" /></svg>;
    case "menu":
      return <svg {...common}><path d="M4 7h16M4 12h16M4 17h16" /></svg>;
    case "package":
      return <svg {...common}><path d="m16.5 9.4-9-5.2M21 16V8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" /><path d="M3.3 7 12 12l8.7-5M12 22V12" /></svg>;
    case "proof":
      return <svg {...common}><path d="M12 3 4.5 6v5.7c0 4.7 3.2 7.7 7.5 9.3 4.3-1.6 7.5-4.6 7.5-9.3V6L12 3Z" /><path d="m8.5 12 2.2 2.2 4.8-5" /></svg>;
    case "shield":
      return <svg {...common}><path d="M12 3 5 6v5c0 4.5 2.8 7.6 7 9 4.2-1.4 7-4.5 7-9V6l-7-3Z" /><path d="M12 8v4M12 16h.01" /></svg>;
    case "terminal":
      return <svg {...common}><path d="m5 7 4 4-4 4M11 17h8" /></svg>;
    case "x":
      return <svg {...common}><path d="M6 6l12 12M18 6 6 18" /></svg>;
  }
}
