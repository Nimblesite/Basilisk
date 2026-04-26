import techdoc from "eleventy-plugin-techdoc";

export default function (eleventyConfig) {
  eleventyConfig.addPlugin(techdoc, {
    site: {
      name: "Basilisk",
      url: "https://basilisk-lang.org",
      description:
        "Strict-by-default Python type checker. Every parameter typed. Every return declared. No escape hatches. Built in Rust.",
      author: "The Basilisk Project",
      themeColor: "#e8500a",
      stylesheet: "/assets/css/styles.css",
      ogImage: "/assets/images/og-image.png",
      organization: {
        name: "Basilisk",
        url: "https://basilisk-lang.org",
        logo: "/assets/images/logo.svg",
        sameAs: [
          "https://github.com/basilisk-lang/basilisk",
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
