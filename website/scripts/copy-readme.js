/**
 * copy-readme.js
 *
 * Copies the root README.md into src/readme.html at build time.
 * The README is the single source of truth — the website page is generated
 * from it automatically on every build.
 *
 * Front-matter is prepended so Eleventy picks it up with the right layout,
 * title, and navigation entry.
 */

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const readmePath = resolve(__dirname, "../../README.md");
const outPath = resolve(__dirname, "../src/readme.html");

const frontmatter = `---
layout: layouts/docs.njk
title: README
description: Crate architecture, diagnostic rules, and development guide for Basilisk.
keywords: basilisk, readme, crate architecture, rust, python type checker
# English-only crate README — no Chinese twin exists, so opt it out of the
# language cluster (no /zh/readme/ hreflang or switcher link, which would 404).
noTranslation: true
eleventyNavigation:
  key: README
  order: 99
permalink: /readme/
---

`;

// The root README uses a repo-relative logo path (`images/basilisk-logo.png`)
// that resolves on GitHub but 404s on the site at /readme/. Rewrite it to the
// site's absolute logo asset so the page renders without a broken image.
const readme = readFileSync(readmePath, "utf8").replace(
  /images\/basilisk-logo\.png/g,
  "/assets/images/logo.svg",
);

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, frontmatter + readme, "utf8");

console.log("✓ README.md copied to src/readme.html");
