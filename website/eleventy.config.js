import { readFileSync, writeFileSync, existsSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import techdoc from "eleventy-plugin-techdoc";
import markdownIt from "markdown-it";
import markdownItAnchor from "markdown-it-anchor";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Patch techdoc templates and behavior with project-owned versions. The plugin
// reads these files after this config loads, so the copies survive a fresh npm
// install without maintaining a fork of the package.
const templateOverrides = [
  ["src/assets/js/mobile-menu.js", "assets/js/mobile-menu.js"],
  ["src/_includes/layouts/base.njk", "templates/layouts/base.njk"],
  ["src/_includes/layouts/blog.njk", "templates/layouts/blog.njk"],
  ["src/_includes/layouts/docs.njk", "templates/layouts/docs.njk"],
  ["src/_includes/pages/feed.njk", "templates/pages/feed.njk"],
  ["src/_includes/pages/robots.txt.njk", "templates/pages/robots.txt.njk"],
  ["src/_includes/pages/sitemap.njk", "templates/pages/sitemap.njk"],
  ["src/_includes/pages/blog/index.njk", "templates/pages/blog/index.njk"],
  ["src/_includes/pages/blog/tags.njk", "templates/pages/blog/tags.njk"],
  ["src/_includes/pages/blog/tags-pages.njk", "templates/pages/blog/tags-pages.njk"],
  ["src/_includes/pages/blog/categories.njk", "templates/pages/blog/categories.njk"],
  ["src/_includes/pages/blog/categories-pages.njk", "templates/pages/blog/categories-pages.njk"],
];
// The copy is LINE-ENDING NORMALIZED, and must stay that way. These overrides
// are working-tree files, so on Windows — where git's `autocrlf` default checks
// them out CRLF — a verbatim copy hands the plugin CRLF template content. Every
// probe below (`patchIndexFrontMatter`'s `^title: .*$` replacements,
// `addSharedProseClass`'s literal `class="docs-content"`, and above all
// `addLocalizedTemplateLang`'s `lang: zh` guard) is written against LF, so on a
// CRLF copy the guard silently missed an existing `lang: zh` and inserted a
// SECOND one — "duplicated mapping key", and the whole site build died on
// Windows while Linux CI stayed green. Normalizing here makes the bytes the
// plugin sees identical on every platform, so one probe cannot pass on Linux and
// fail on Windows. On Linux this is a no-op: the files are already LF.
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

function addSharedProseClass(path, content) {
  return path === "_includes/layouts/api.njk"
    ? content.replace('class="docs-content"', 'class="docs-content prose"')
    : content;
}

function addLocalizedTemplateLang(path, content) {
  return path.startsWith("zh/blog/") && !content.includes("\nlang: zh\n")
    ? content.replace("layout: layouts/base.njk", "layout: layouts/base.njk\nlang: zh")
    : content;
}

export default function (eleventyConfig) {
  const originalAddTemplate = eleventyConfig.addTemplate.bind(eleventyConfig);
  eleventyConfig.addTemplate = (virtualInputPath, content, data) =>
    originalAddTemplate(
      virtualInputPath,
      patchIndexFrontMatter(
        virtualInputPath,
        addSharedProseClass(
          virtualInputPath,
          addLocalizedTemplateLang(virtualInputPath, content)
        )
      ),
      data
    );

  eleventyConfig.addPlugin(techdoc, {
    site: {
      name: "Basilisk",
      url: "https://www.basilisk-python.dev",
      description:
        "The only Python type checker with a perfect 100% score on the official python/typing conformance results. Open-source, strict-by-default language server built in Rust — type checking, autocomplete, refactoring, debugging, and profiling in VS Code, Cursor, Zed, and Neovim.",
      author: "The Basilisk Project",
      themeColor: "#e8500a",
      stylesheet: "/assets/css/styles.css",
      ogImage: "/assets/images/og-image.png",
      organization: {
        name: "Basilisk",
        url: "https://www.basilisk-python.dev",
        logo: "/assets/images/favicon.png",
        sameAs: [
          "https://github.com/Nimblesite/Basilisk",
        ],
      },
    },
    features: {
      blog: true,
      docs: true,
      darkMode: true,
      i18n: true,
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

  // Preserve CJK headings in fragment identifiers. Techdoc's default slugger
  // strips all non-ASCII letters, which produces empty and numeric-only IDs on
  // Chinese prose pages and breaks their heading permalinks.
  const markdown = markdownIt({ html: true, breaks: false, linkify: true }).use(
    markdownItAnchor,
    {
      level: [1, 2, 3, 4],
      permalink: markdownItAnchor.permalink.headerLink(),
      slugify: (value) =>
        value
          .normalize("NFKC")
          .toLowerCase()
          .trim()
          .replace(/[^\p{Letter}\p{Number}_-]+/gu, "-")
          .replace(/^-+|-+$/g, ""),
    }
  );
  // Eleventy executes plugins after the project config returns, so register
  // this override as the next plugin to ensure it runs after techdoc's default.
  eleventyConfig.addPlugin((config) => config.setLibrary("md", markdown));

  eleventyConfig.addPassthroughCopy("src/assets");
  eleventyConfig.addPassthroughCopy({
    "node_modules/monaco-editor/min/vs": "assets/vendor/monaco/vs",
    "node_modules/lz-string/libs/lz-string.min.js": "assets/vendor/lz-string.min.js",
  });
  eleventyConfig.addPassthroughCopy("src/CNAME");

  // [Author pages] Posts written by a given author, matched on the post's
  // `author` front-matter string == the author's `name` in _data/authors.json.
  // Newest first, English posts only (Chinese posts carry their own byline).
  eleventyConfig.addFilter("authorPosts", (posts, authorName) =>
    (posts || [])
      .filter((p) => p.data.author === authorName && !p.url.startsWith("/zh/"))
      .sort((a, b) => b.date - a.date)
  );

  // [Author pages] Plain-text truncation for meta descriptions built from a bio.
  eleventyConfig.addFilter("truncate", (str, len) => {
    const s = String(str || "");
    if (s.length <= len) return s;
    return s.slice(0, s.lastIndexOf(" ", len)).trimEnd() + "…";
  });

  const categoryLabels = {
    en: { announcements: "Announcements", "deep-dives": "Deep dives" },
    zh: { announcements: "公告", "deep-dives": "深度解析" },
  };
  eleventyConfig.addFilter("blogCategoryLabel", (category, lang = "en") =>
    categoryLabels[lang]?.[category] ||
    String(category || "").replaceAll("-", " ")
  );

  const docsNavActive = (node, currentUrl) => {
    const current = String(currentUrl || "").replace(/^\/zh(?=\/)/, "");
    if (
      node?.kind === "rules" &&
      (current.startsWith("/docs/rules/") || current.startsWith("/errors/"))
    ) {
      return true;
    }
    if (node?.url === current) return true;
    return [...(node?.items || []), ...(node?.children || [])].some((child) =>
      docsNavActive(child, current)
    );
  };
  eleventyConfig.addFilter("docsNavActive", docsNavActive);

  // Base layout guard: only advertise a language alternate when Eleventy
  // actually generated that URL. This prevents hreflang and switcher 404s on
  // English-only docs, author profiles, benchmarks, and diagnostic pages.
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
    templateFormats: ["md", "njk", "html"],
    markdownTemplateEngine: "njk",
    htmlTemplateEngine: "njk",
  };
}
