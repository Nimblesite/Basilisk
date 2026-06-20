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

// SEO metadata for the plugin-generated blog / tags / categories index pages.
// The techdoc plugin registers these as virtual templates (node_modules-only,
// no source file to edit) whose default front matter only sets a bare title
// ("Blog", "Tags", "Categories") and no description, so every index inherits the
// generic site description — non-unique and too short for SEO. We can't add a
// same-path source override (Eleventy errors when a file collides with a virtual
// template), and the plugin registers its templates AFTER this config callback
// returns, so the virtualTemplates map is empty here. Instead we wrap
// addTemplate() below and patch each index template's front matter as the plugin
// registers it — project-level, leaving node_modules untouched.
// [SEO index metadata override]
const indexSeo = {
  "blog/index.njk": {
    title: "Basilisk Blog — Python Type-Checking News & Releases",
    description:
      "News, releases, and deep dives from Basilisk — the open-source, strict-by-default Python language server in Rust for VS Code, Cursor, Zed, and Neovim.",
  },
  "blog/tags.njk": {
    title: "Blog Tags — Browse Basilisk Posts by Topic",
    description:
      "Browse Basilisk blog posts by tag to find writing on Python type checking, strict typing, LSP features, refactoring, debugging, profiling, and release notes.",
  },
  "blog/categories.njk": {
    title: "Blog Categories — Browse Basilisk Posts by Section",
    description:
      "Browse Basilisk blog posts by category to explore announcements, deep dives, and release notes for the strict-by-default Python language server built in Rust.",
  },
  "zh/blog/index.njk": {
    title: "Basilisk 博客 — Python 类型检查动态与版本发布",
    description:
      "来自 Basilisk 项目的动态、版本发布与深入解析——一个用 Rust 构建、严格优先的开源 Python 语言服务器，支持 VS Code、Cursor、Zed 与 Neovim。",
  },
  "zh/blog/tags.njk": {
    title: "博客标签 — 按主题浏览 Basilisk 文章",
    description:
      "按标签浏览 Basilisk 博客文章，查找有关 Python 类型检查、严格类型、LSP 功能、重构、调试、性能分析与版本发布说明的内容，按主题快速定位。",
  },
  "zh/blog/categories.njk": {
    title: "博客分类 — 按栏目浏览 Basilisk 文章",
    description:
      "按分类浏览 Basilisk 博客文章，探索这个用 Rust 构建、严格优先的开源 Python 语言服务器的公告、深入解析与版本发布说明等栏目内容。",
  },
};

function patchIndexFrontMatter(path, content) {
  const meta = indexSeo[path];
  if (!meta) {
    return content;
  }
  return content
    .replace(/^title: .*$/m, `title: "${meta.title}"`)
    .replace(/^(title: .*)$/m, `$1\ndescription: "${meta.description}"`);
}

export default function (eleventyConfig) {
  const originalAddTemplate = eleventyConfig.addTemplate.bind(eleventyConfig);
  eleventyConfig.addTemplate = (virtualInputPath, content, data) =>
    originalAddTemplate(
      virtualInputPath,
      patchIndexFrontMatter(virtualInputPath, content),
      data
    );

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
    // Register the languages the site actually ships so the base layout emits a
    // complete hreflang cluster (en + zh + x-default) and og:locale:alternate.
    // Without this, supportedLanguages defaults to ['en'] and the Chinese pages
    // are never declared as alternates — Google can't connect /  ⇄  /zh/.
    i18n: {
      defaultLanguage: "en",
      languages: ["en", "zh"],
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
