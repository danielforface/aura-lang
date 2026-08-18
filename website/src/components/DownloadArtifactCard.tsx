"use client";

import { useEffect, useState } from "react";

import { Icon } from "@/components/Icon";

type Availability = "checking" | "available" | "missing";

type Props = {
  title: string;
  kind: string;
  description: string;
  href: string;
  filename: string;
  detail?: string;
  primary?: boolean;
};

export function DownloadArtifactCard({ title, kind, description, href, filename, detail, primary = false }: Props) {
  const [availability, setAvailability] = useState<Availability>("checking");

  useEffect(() => {
    const controller = new AbortController();

    async function check() {
      try {
        const response = await fetch(href, {
          method: "HEAD",
          cache: "no-store",
          signal: controller.signal,
        });
        setAvailability(response.ok ? "available" : "missing");
      } catch {
        if (!controller.signal.aborted) setAvailability("missing");
      }
    }

    void check();
    return () => controller.abort();
  }, [href]);

  const statusLabel = availability === "available"
    ? "Ready to download"
    : availability === "missing"
      ? "Not staged on this deployment"
      : "Checking artifact";

  return (
    <article className={primary ? "download-card is-primary" : "download-card"}>
      <div className="download-card-topline">
        <span className="download-kind">{kind}</span>
        <span className={`download-availability is-${availability}`}>
          <span className="download-availability-dot" />
          {statusLabel}
        </span>
      </div>
      <h3>{title}</h3>
      <p>{description}</p>
      {detail ? <p className="download-detail">{detail}</p> : null}
      <div className="download-file-row">
        <code>{filename}</code>
        {availability === "available" ? (
          <a className="button-primary" href={href} download>
            Download <Icon name="arrow" size={15} />
          </a>
        ) : (
          <span className="button-disabled" aria-disabled="true">
            {availability === "checking" ? "Checking…" : "Not staged"}
          </span>
        )}
      </div>
    </article>
  );
}
