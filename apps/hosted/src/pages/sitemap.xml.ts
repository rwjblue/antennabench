import type { APIRoute } from "astro";

const paths = ["/", "/how-it-works/", "/sample-report/"];

export const GET: APIRoute = ({ site }) => {
  if (site === undefined) {
    throw new Error("Astro site URL is required for sitemap generation");
  }
  const urls = paths
    .map((path) => `  <url><loc>${new URL(path, site).href}</loc></url>`)
    .join("\n");
  return new Response(
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`,
    { headers: { "content-type": "application/xml; charset=utf-8" } },
  );
};
