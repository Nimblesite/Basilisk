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
eleventyNavigation:
  key: README
  order: 99
permalink: /readme/
---

`;

const readme = readFileSync(readmePath, "utf8");

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, frontmatter + readme, "utf8");

console.log("✓ README.md copied to src/readme.html");
