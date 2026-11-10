import { readFileSync, writeFileSync, existsSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import techdoc from "eleventy-plugin-techdoc";
import withdrawal from "./src/_data/withdrawal.json" with { type: "json" };

const __dirname = dirname(fileURLToPath(import.meta.url));

// Patch techdoc templates with project-owned versions. The plugin reads these
// files after this config loads, so the copies survive a fresh npm install
// without maintaining a fork of the package.
const templateOverrides = [
  ["src/assets/js/mobile-menu.js", "assets/js/mobile-menu.js"],
  ["src/_includes/layouts/base.njk", "templates/layouts/base.njk"],
  ["src/_includes/pages/robots.txt.njk", "templates/pages/robots.txt.njk"],
  ["src/_includes/pages/sitemap.njk", "templates/pages/sitemap.njk"],
  ["src/_includes/pages/llms.txt.njk", "templates/pages/llms.txt.njk"],
  ["src/_includes/pages/feed.njk", "templates/pages/feed.njk"],
];
// The copy is LINE-ENDING NORMALIZED, and must stay that way. These overrides
// are working-tree files, so on Windows — where git's `autocrlf` default checks
// them out CRLF — a verbatim copy hands the plugin CRLF template content, and
// every literal probe written against LF silently misses. Normalizing here makes
// the bytes the plugin sees identical on every platform. On Linux it is a no-op.
const toLf = (text) => text.replace(/\r\n/g, "\n");

for (const [source, target] of templateOverrides) {
  const localOverride = join(__dirname, source);
  const pluginTarget = join(
    __dirname,
    "node_modules/eleventy-plugin-techdoc",
    target
  );
  if (existsSync(localOverride)) {
    writeFileSync(pluginTarget, toLf(readFileSync(localOverride, "utf-8")));
  }
}

export default function (eleventyConfig) {
  eleventyConfig.addPlugin(techdoc, {
    // Implements [WITHDRAWAL-COPY]: the site's own description is the approved
    // one-line copy, generated from the messaging spec.
    site: {
      name: "Basilisk",
      url: "https://www.basilisk-python.dev",
      description: withdrawal.line,
      author: "The Basilisk Project",
      themeColor: "#e8500a",
      stylesheet: "/assets/css/styles.css",
      organization: {
        name: "Basilisk",
        url: "https://www.basilisk-python.dev",
        logo: "/assets/images/favicon.png",
        sameAs: ["https://github.com/Nimblesite/Basilisk"],
      },
    },
    // The site serves one statement and a notice at every retired URL. There is
    // no blog, no docs tree, and no second language to translate into: the
    // approved copy exists only in English ([WITHDRAWAL-COPY]).
    features: {
      blog: false,
      docs: false,
      darkMode: true,
      i18n: false,
    },
  });

  eleventyConfig.addPassthroughCopy("src/assets");
  eleventyConfig.addPassthroughCopy("src/CNAME");

  // Base layout guard: only advertise a language alternate when Eleventy
  // actually generated that URL.
  eleventyConfig.addFilter("hasPageUrl", (pages, url) =>
    (pages || []).some((page) => page.url === url)
  );

  return {
    dir: {
      input: "src",
      output: "_site",
      includes: "_includes",
      data: "_data",
    },
    templateFormats: ["njk", "html"],
    markdownTemplateEngine: "njk",
    htmlTemplateEngine: "njk",
  };
}
