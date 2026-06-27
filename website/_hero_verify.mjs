import { chromium } from 'playwright-core';
import { execSync } from 'node:child_process';

const chromePath = execSync(
  "node -e \"console.log(require('playwright-core').chromium.executablePath())\"",
  { cwd: process.cwd() }
).toString().trim();

const BASE = 'http://localhost:8199/';
const OUT = '/private/tmp/claude-501/-Users-christianfindlay-Documents-Code-Basilisk/179e3680-02ec-4d6e-9c31-c7911d352e54/scratchpad';

const viewports = [
  { name: 'desktop', width: 1440, height: 900 },
  { name: 'laptop',  width: 1280, height: 800 },
  { name: 'mobile',  width: 390,  height: 844 },
];

const browser = await chromium.launch({ executablePath: chromePath, headless: true });
for (const vp of viewports) {
  const page = await browser.newPage({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: 2 });
  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  const m = await page.evaluate(() => {
    const q = (s) => document.querySelector(s);
    const r = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { w: Math.round(b.width), h: Math.round(b.height), top: Math.round(b.top), bottom: Math.round(b.bottom) }; };
    const img = q('.hero__shot');
    return {
      vh: window.innerHeight,
      hero: r(q('.hero')),
      split: r(q('.hero__split')),
      text: r(q('.hero__split > div:first-child')),
      code: r(q('.hero__code')),
      shot: r(img),
      shotNatural: img ? { nw: img.naturalWidth, nh: img.naturalHeight, ratio: (img.naturalWidth/img.naturalHeight).toFixed(3) } : null,
      shotDisplayRatio: img ? (img.getBoundingClientRect().width / img.getBoundingClientRect().height).toFixed(3) : null,
      metaLines: (() => { const meta = q('.hero__meta'); if (!meta) return null; const spans=[...meta.querySelectorAll('span')]; const tops=new Set(spans.map(s=>Math.round(s.getBoundingClientRect().top))); return tops.size; })(),
    };
  });
  console.log(`\n=== ${vp.name} ${vp.width}x${vp.height} ===`);
  console.log(JSON.stringify(m, null, 2));
  await page.screenshot({ path: `${OUT}/hero-${vp.name}.png`, fullPage: false });
  // also a full-page shot of the hero region only
  await page.close();
}
await browser.close();
console.log('\nDONE');
