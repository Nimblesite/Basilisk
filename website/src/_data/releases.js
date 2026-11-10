// Eleventy global data: the Basilisk GitHub Releases, fetched FRESH at every
// build from the public GitHub REST API — never hand-maintained. This mirrors
// the build-time data pattern of _data/conformance.js and _data/benchmarks.js:
// everything the /docs/releases/ page shows is whatever the API returns at build
// time (tag, title, date, release notes rendered from the release's markdown
// body, and downloadable assets).
//
// Drafts are excluded (not yet published). Prereleases are kept and badged.
//
// The build NEVER fails on a network/API error: exactly like conformance.js it
// degrades to `{ hasData: false }` and the page renders an empty state linking
// to GitHub, so an offline dev build or a rate-limited CI run still produces a
// valid site. When `GITHUB_TOKEN`/`GH_TOKEN` is present (CI) it is used to raise
// the API rate limit; the public, unauthenticated path works too.
import markdownIt from "markdown-it";

const OWNER = "Nimblesite";
const REPO = "Basilisk";
const API = `https://api.github.com/repos/${OWNER}/${REPO}/releases?per_page=100`;
const RELEASES_URL = `https://github.com/${OWNER}/${REPO}/releases`;

// Release notes are authored by the maintainers (trusted), so raw HTML is
// allowed. `breaks: true` matches how GitHub itself renders release bodies.
const md = markdownIt({ html: true, linkify: true, breaks: true });

// Withdrawn-claim redaction. Release notes arrive verbatim from GitHub, and some
// historical entries quote the conformance result that has since been retracted.
// [CHKARCH-CONFORMANCE] forbids publishing, quoting, or marketing any conformance
// figure, so rendering those lines unchanged would keep republishing a claim we
// have withdrawn. Any line pairing a conformance subject with a figure is replaced
// by a visible marker — nothing is silently dropped, the marker links to the
// correction, and every release heading still links to the unmodified GitHub
// release so the original wording stays one click away.
const CLAIM_SUBJECT = /conformance|conformant/i;
const CLAIM_FIGURE = /\d+(?:\.\d+)?\s*%|\b\d{1,4}\s*\/\s*\d{1,4}\b|\b\d+\s+false\s+positives?\b/i;
const LIST_MARKER = /^(\s*(?:[-*+]|\d+\.)\s+)/;
const REDACTION = "*[withdrawn conformance claim redacted — see the correction](/docs/conformance/)*";

// Replace each claim-bearing line with the marker, preserving its list marker so
// the surrounding changelog structure still renders.
function redactWithdrawnClaims(markdown) {
  return markdown
    .split(/\r?\n/)
    .map((line) => {
      if (!CLAIM_SUBJECT.test(line) || !CLAIM_FIGURE.test(line)) return line;
      const marker = line.match(LIST_MARKER);
      return `${marker ? marker[1] : ""}${REDACTION}`;
    })
    .join("\n");
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

// "2026-06-23T10:16:43Z" -> "Jun 23, 2026". UTC getters keep the output
// deterministic regardless of the build machine's timezone.
function formatDate(iso) {
  if (!iso) return null;
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? iso
    : `${MONTHS[date.getUTCMonth()]} ${date.getUTCDate()}, ${date.getUTCFullYear()}`;
}

// Bytes -> "1.2 MB" style, base-1024.
function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const exp = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exp;
  return `${exp === 0 ? value : Math.round(value * 10) / 10} ${units[exp]}`;
}

// Pull the `rel="next"` URL out of a GitHub `Link` response header (string
// splitting, no regex). Returns null when there is no next page.
function nextPageUrl(linkHeader) {
  if (!linkHeader) return null;
  for (const part of linkHeader.split(",")) {
    const [target, ...attrs] = part.split(";");
    if (attrs.some((attr) => attr.trim() === 'rel="next"')) {
      return target.trim().slice(1, -1); // strip the surrounding < >
    }
  }
  return null;
}

async function fetchAllReleases() {
  const headers = {
    Accept: "application/vnd.github+json",
    "User-Agent": `${OWNER}-${REPO}-website-build`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
  const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;
  if (token) headers.Authorization = `Bearer ${token}`;

  const releases = [];
  let url = API;
  while (url) {
    const response = await fetch(url, { headers });
    if (!response.ok) {
      throw new Error(`GitHub API ${response.status} ${response.statusText}`);
    }
    releases.push(...(await response.json()));
    url = nextPageUrl(response.headers.get("link"));
  }
  return releases;
}

// Shape one API release into the flat record the template renders.
function toRecord(release) {
  return {
    tag: release.tag_name,
    name: release.name || release.tag_name,
    url: release.html_url,
    date: formatDate(release.published_at || release.created_at),
    dateIso: release.published_at || release.created_at,
    prerelease: release.prerelease === true,
    bodyHtml: release.body ? md.render(redactWithdrawnClaims(release.body)) : "",
    assets: (release.assets || []).map((asset) => ({
      name: asset.name,
      url: asset.browser_download_url,
      size: formatBytes(asset.size),
      downloads: asset.download_count || 0,
    })),
  };
}

const EMPTY = { hasData: false, releasesUrl: RELEASES_URL, count: 0, releases: [] };

export default async function () {
  try {
    const published = (await fetchAllReleases())
      .filter((release) => release.draft !== true)
      .sort((a, b) => new Date(b.published_at || b.created_at) - new Date(a.published_at || a.created_at))
      .map(toRecord);

    if (!published.length) return EMPTY;

    return {
      hasData: true,
      releasesUrl: RELEASES_URL,
      count: published.length,
      latest: published[0],
      releases: published,
    };
  } catch (error) {
    // Degrade gracefully — a broken build is worse than a stale releases page.
    console.warn(`⚠ releases.js: ${error.message} — rendering empty state`);
    return EMPTY;
  }
}
