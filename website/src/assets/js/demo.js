/* Basilisk website — interactive demo JS */

/* ── Demo tab switching ────────────────────────────────────── */
document.querySelectorAll(".demo-tabs").forEach((tabGroup) => {
  const tabs   = tabGroup.querySelectorAll("[data-tab]");
  const panels = tabGroup
    .closest(".demo-container")
    .querySelectorAll("[data-panel]");

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      const target = tab.dataset.tab;

      tabs.forEach((t) => t.classList.toggle("active", t === tab));
      panels.forEach((p) =>
        p.classList.toggle("active", p.dataset.panel === target)
      );
    });
  });
});

/* ── Copy-to-clipboard for install commands ────────────────── */
document.querySelectorAll("[data-copy]").forEach((btn) => {
  btn.addEventListener("click", () => {
    const text = btn.dataset.copy;
    navigator.clipboard.writeText(text).then(() => {
      const icon = btn.querySelector(".copy-icon");
      const check = btn.querySelector(".copy-check");
      if (icon)  icon.style.display  = "none";
      if (check) check.style.display = "inline";
      setTimeout(() => {
        if (icon)  icon.style.display  = "";
        if (check) check.style.display = "none";
      }, 1800);
    });
  });
});

/* ── Docs active nav link ──────────────────────────────────── */
const currentPath = window.location.pathname;
document.querySelectorAll(".docs-nav-link").forEach((link) => {
  const href = link.getAttribute("href");
  if (href && (currentPath === href || currentPath.startsWith(href.replace(/\/$/, "") + "/"))) {
    link.classList.add("active");
  }
});

/* ── Smooth scroll for anchor links ───────────────────────── */
document.querySelectorAll('a[href^="#"]').forEach((anchor) => {
  anchor.addEventListener("click", (e) => {
    const target = document.querySelector(anchor.getAttribute("href"));
    if (target) {
      e.preventDefault();
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  });
});
