import { readFileSync, writeFileSync, existsSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import techdoc from "eleventy-plugin-techdoc";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Patch the techdoc plugin's base layout with our custom version (adds favicon + logo)
const localOverride = join(__dirname, "src/_includes/layouts/base.njk");
const pluginTarget = join(__dirname, "node_modules/eleventy-plugin-techdoc/templates/layouts/base.njk");

if (existsSync(localOverride)) {
  writeFileSync(pluginTarget, readFileSync(localOverride, "utf-8"));
}

export default function (eleventyConfig) {
  eleventyConfig.addPlugin(techdoc, {
    site: {
      name: "Basilisk",
      url: "https://www.basilisk-python.dev",
      description:
        "Open-source, strict-by-default Python language server built in Rust. Type checking, autocomplete, refactoring, debugging, and profiling — in VS Code, Cursor, Zed, and Neovim.",
      author: "The Basilisk Project",
      themeColor: "#e8500a",
      stylesheet: "/assets/css/styles.css",
      ogImage: "/assets/images/og-image.png",
      organization: {
        name: "Basilisk",
        url: "https://www.basilisk-python.dev",
        logo: "/assets/images/logo.svg",
        sameAs: [
          "https://github.com/Nimblesite/Basilisk",
        ],
      },
    },
    features: {
      blog: true,
      docs: true,
      darkMode: true,
      i18n: false,
    },
  });

  eleventyConfig.addPassthroughCopy("src/assets");
  eleventyConfig.addPassthroughCopy("src/CNAME");
  eleventyConfig.addPassthroughCopy("src/robots.txt");

  return {
    dir: {
      input: "src",
      output: "_site",
      includes: "_includes",
      data: "_data",
    },
    templateFormats: ["md", "njk", "html"],
    markdownTemplateEngine: "njk",
    htmlTemplateEngine: "njk",
  };
}
