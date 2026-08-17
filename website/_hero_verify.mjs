import { chromium } from 'playwright-core';

const chromePath = chromium.executablePath();
const BASE = 'http://localhost:8199/';
const OUT = '/private/tmp/claude-501/-Users-christianfindlay-Documents-Code-Basilisk/179e3680-02ec-4d6e-9c31-c7911d352e54/scratchpad';

const viewports = [
  { name: 'wide',    width: 2560, height: 1440, dsf: 1 },
  { name: 'desktop', width: 1440, height: 900,  dsf: 1 },
  { name: 'mobile',  width: 390,  height: 844,  dsf: 2 },
];

const browser = await chromium.launch({ executablePath: chromePath, headless: true });
for (const vp of viewports) {
  const page = await browser.newPage({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.dsf });
  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.waitForTimeout(250);
  const m = await page.evaluate(() => {
    const q = (s) => document.querySelector(s);
    const r = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { w: Math.round(b.width), h: Math.round(b.height), top: Math.round(b.top), left: Math.round(b.left), right: Math.round(b.right), bottom: Math.round(b.bottom) }; };
    const vw = window.innerWidth, vh = window.innerHeight;
    const hero = q('.hero');
    const split = q('.hero__split');
    const img = q('.hero__shot');
    const sb = split ? split.getBoundingClientRect() : null;
    return {
      vw, vh,
      heroH: hero ? Math.round(hero.getBoundingClientRect().height) : null,
      split: r(split),
      // side gutter = empty space from viewport edge to the content block
      gutterLeft: sb ? Math.round(sb.left) : null,
      gutterRight: sb ? Math.round(vw - sb.right) : null,
      // vertical gap from hero top/bottom to the content block (within the hero)
      gapTop: (split && hero) ? Math.round(sb.top - hero.getBoundingClientRect().top) : null,
      gapBottom: (split && hero) ? Math.round(hero.getBoundingClientRect().bottom - sb.bottom) : null,
      text: r(q('.hero__split > div:first-child')),
      shot: r(img),
      shotDisplayRatio: img ? (img.getBoundingClientRect().width / img.getBoundingClientRect().height).toFixed(3) : null,
      shotNatRatio: img ? (img.naturalWidth / img.naturalHeight).toFixed(3) : null,
      metaLines: (() => { const meta = q('.hero__meta'); if (!meta) return null; const tops = new Set([...meta.querySelectorAll('span')].map(s => Math.round(s.getBoundingClientRect().top))); return tops.size; })(),
    };
  });
  console.log(`\n=== ${vp.name} ${vp.width}x${vp.height} ===`);
  console.log(JSON.stringify(m));
  await page.screenshot({ path: `${OUT}/hero-${vp.name}.png`, fullPage: false });
  await page.close();
}
await browser.close();
console.log('\nDONE');
