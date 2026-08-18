import { DOC_GROUPS } from "./siteData";

export type DocsNavItem = {
  slug: string;
  title: string;
  group: string;
};

export const docsNav: DocsNavItem[] = DOC_GROUPS.flatMap((group) =>
  group.items.map(([slug, title]) => ({ slug, title, group: group.label })),
);
