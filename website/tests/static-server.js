// Dependency-free static file server for the built `_site/` output.
// Used by Playwright's `webServer` so e2e tests run against the real,
// production-built site (no Eleventy dev server / live-reload in the loop).
//
// Implements [WEBSITE-E2E-SMOKE]: serve the static build so navigation
// smoke tests can exercise it. See docs/specs/WEBSITE-E2E-SPEC.md.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { join, normalize, extname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../_site/", import.meta.url));
const PORT = Number(process.env.PORT ?? 8099);

const MIME = new Map([
  [".html", "text/html; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".svg", "image/svg+xml"],
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".webp", "image/webp"],
  [".xml", "application/xml; charset=utf-8"],
  [".txt", "text/plain; charset=utf-8"],
  [".woff2", "font/woff2"],
]);

// Resolve a request path to a file on disk, mapping directory URLs to their
// `index.html` and rejecting any path that escapes the `_site/` root.
async function resolvePath(urlPath) {
  const decoded = decodeURIComponent(urlPath.split("?")[0].split("#")[0]);
  const rel = normalize(decoded).replace(/^(\.\.[/\\])+/, "");
  let candidate = join(ROOT, rel);
  if (!candidate.startsWith(ROOT)) {
    return null;
  }
  if (candidate.endsWith("/") || candidate === ROOT.replace(/[/\\]$/, "")) {
    candidate = join(candidate, "index.html");
  }
  try {
    const info = await stat(candidate);
    return info.isDirectory() ? join(candidate, "index.html") : candidate;
  } catch {
    return candidate.endsWith(".html") ? candidate : `${candidate}/index.html`;
  }
}

const server = createServer(async (req, res) => {
  const filePath = await resolvePath(req.url ?? "/");
  if (!filePath) {
    res.writeHead(403).end("Forbidden");
    return;
  }
  try {
    const body = await readFile(filePath);
    res.writeHead(200, {
      "content-type": MIME.get(extname(filePath)) ?? "application/octet-stream",
    });
    res.end(body);
  } catch {
    res.writeHead(404, { "content-type": "text/html; charset=utf-8" });
    res.end("<h1>404</h1>");
  }
});

server.listen(PORT, () => {
  process.stdout.write(`static-server: serving _site on http://127.0.0.1:${PORT}\n`);
});
