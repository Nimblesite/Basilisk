/** Project-owned mobile navigation: site links and docs navigation are separate. */
export function initMobileMenu() {
  const siteToggle = document.getElementById("mobile-menu-toggle");
  const siteNavigation = document.getElementById("site-navigation");
  const docsToggle = document.getElementById("docs-menu-toggle");
  const docsNavigation = document.getElementById("docs-sidebar");

  siteToggle?.addEventListener("click", () => {
    const expanded = siteToggle.getAttribute("aria-expanded") === "true";
    siteToggle.setAttribute("aria-expanded", String(!expanded));
    siteNavigation?.classList.toggle("open", !expanded);
  });

  docsToggle?.addEventListener("click", () => {
    const expanded = docsToggle.getAttribute("aria-expanded") === "true";
    docsToggle.setAttribute("aria-expanded", String(!expanded));
    docsNavigation?.classList.toggle("open", !expanded);
  });
}
