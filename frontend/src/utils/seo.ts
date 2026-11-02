import { useEffect } from "react";
import site from "../../seo.config.json";

export function useSeo(title: string, description: string, noIndex = false) {
  useEffect(() => {
    document.title = title;
    const values = [
      ["name", "description", description],
      ["name", "robots", noIndex ? "noindex, follow" : "index, follow"],
      ["property", "og:title", title],
      ["property", "og:description", description],
      ["name", "twitter:title", title],
      ["name", "twitter:description", description],
    ];
    values.forEach(([attribute, key, content]) => {
      let meta = document.head.querySelector<HTMLMetaElement>(`meta[${attribute}="${key}"]`);
      if (!meta) {
        meta = document.createElement("meta");
        meta.setAttribute(attribute!, key!);
        document.head.appendChild(meta);
      }
      meta.content = content!;
    });
    if (site.publicUrl) {
      const url = new URL(window.location.pathname, site.publicUrl).href;
      let canonical = document.head.querySelector<HTMLLinkElement>('link[rel="canonical"]');
      if (!canonical) {
        canonical = document.createElement("link");
        canonical.rel = "canonical";
        document.head.appendChild(canonical);
      }
      canonical.href = url;
    }
  }, [title, description, noIndex]);
}

