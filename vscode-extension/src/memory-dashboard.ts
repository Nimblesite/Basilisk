// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Memory dashboard webview for Basilisk memory profiling.
 *
 * Provides:
 * - Summary cards: current memory, peak memory, gc objects, gc counts
 * - Timeline chart: memory usage over time (Canvas 2D line chart)
 * - Top allocations table: file, line, size, count — click to navigate
 * - Leak confidence badges: Definite, High, Medium, Low
 * - Dual heat map mode toggle: CPU (orange) + Memory (purple)
 *
 * All data comes from the LSP via memory snapshot/diff commands.
 * This module handles only the client-side visualization.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import type { MemoryAllocation } from "./memory-decorations";
import {
  PROFILER_CSS_VARS,
  PROFILER_CSS_RESET,
  PROFILER_CSS_CARDS,
  PROFILER_CSS_TABLE,
  PROFILER_CSS_HEADING,
  PROFILER_JS_UTILS,
  formatBytes as formatBytesShared,
} from "./profiler-styles";

// ── Types ─────────────────────────────────────────────────────────────────

/** A single time-series data point for the memory timeline. */
export interface MemoryTimelinePoint {
  timestamp: number;
  currentMemory: number;
  peakMemory: number;
  gcObjects: number;
}

/** Snapshot data extended with timeline and gc info. */
export interface MemoryDashboardSnapshot {
  memorySessionId: string;
  snapshotId: string;
  currentMemory: number;
  peakMemory: number;
  gcObjects: number;
  gcCounts: number[];
  topAllocations: MemoryAllocation[];
  timeline: MemoryTimelinePoint[];
}

/** A suspected leak from a snapshot diff. */
export interface SuspectedLeak {
  file: string;
  line: number;
  sizeGrowth: number;
  countGrowth: number;
  currentSize: number;
  currentCount: number;
  confidence: "definite" | "high" | "medium" | "low";
  reason: string;
}

/** Diff data comparing two memory snapshots. */
export interface MemoryDiffData {
  totalGrowth: number;
  totalFreed: number;
  netGrowth: number;
  suspectedLeaks: SuspectedLeak[];
  grownAllocations: MemoryAllocation[];
}

// ── State ─────────────────────────────────────────────────────────────────

let memoryDashboardPanel: vscode.WebviewPanel | undefined;

// ── Public API ────────────────────────────────────────────────────────────

/**
 * Open or reveal the memory dashboard webview.
 * If diffData is provided, the leak analysis section is shown.
 */
export function openMemoryDashboard(
  snapshotData: MemoryDashboardSnapshot,
  diffData?: MemoryDiffData,
): void {
  if (memoryDashboardPanel !== undefined) {
    memoryDashboardPanel.reveal(vscode.ViewColumn.Beside);
  } else {
    memoryDashboardPanel = vscode.window.createWebviewPanel(
      "basilisk.memoryDashboard",
      "Basilisk Memory Dashboard",
      vscode.ViewColumn.Beside,
      { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] },
    );
    memoryDashboardPanel.onDidDispose(() => { memoryDashboardPanel = undefined; });
  }

  memoryDashboardPanel.webview.html = buildMemoryDashboardHtml(snapshotData, diffData);

  memoryDashboardPanel.webview.onDidReceiveMessage(
    (msg: { type: string; file?: string; line?: number }) => {
      if (msg.type === "navigateToSource" && msg.file !== undefined && msg.line !== undefined) {
        const uri = vscode.Uri.file(msg.file);
        const position = new vscode.Position(msg.line - 1, 0);
        void vscode.window.showTextDocument(uri, {
          selection: new vscode.Range(position, position),
          viewColumn: vscode.ViewColumn.One,
        });
      }
    },
  );

  const leakCount = diffData?.suspectedLeaks.length ?? 0;
  Logger.info(
    `Memory dashboard opened: ${formatBytesShared(snapshotData.currentMemory)} current, ` +
    `${snapshotData.topAllocations.length} allocations, ${leakCount} suspected leaks`,
  );
}

/** Dispose the memory dashboard panel if open. */
export function disposeMemoryDashboard(): void {
  if (memoryDashboardPanel !== undefined) {
    memoryDashboardPanel.dispose();
    memoryDashboardPanel = undefined;
  }
}

// ── HTML builder ──────────────────────────────────────────────────────────

function buildMemoryDashboardHtml(
  snapshot: MemoryDashboardSnapshot,
  diff?: MemoryDiffData,
): string {
  const css = buildDashboardHeadCss();
  const body = buildDashboardBodyHtml();
  const script = buildDashboardScriptTag(snapshot, diff);
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Basilisk Memory Dashboard</title>
  <style>${css}</style>
</head>
<body>
  ${body}
  <script>${script}</script>
</body>
</html>`;
}

function buildDashboardHeadCss(): string {
  return `${PROFILER_CSS_VARS}${PROFILER_CSS_RESET}
    body { padding: 16px; overflow-y: auto; }
    h1 .accent { color: var(--prof-mem-critical); }
    .card .value.purple { color: var(--prof-mem-critical); }
    ${PROFILER_CSS_HEADING}${PROFILER_CSS_CARDS}${PROFILER_CSS_TABLE}${buildDashboardCss()}`;
}

function buildDashboardBodyHtml(): string {
  return `
  <h1><span class="accent">BASILISK</span> MEMORY</h1>
  ${buildSummaryCardsHtml()}
  <h2>Memory Timeline</h2>
  <div class="timeline-container">
    <canvas class="timeline-canvas" id="timeline-canvas"></canvas>
    <div class="crosshair-label" id="crosshair-label"></div>
  </div>
  <h2>Heat Map Mode</h2>
  <div class="toggle-row">
    <button class="toggle-btn active-mem" id="btn-mem">Memory (purple)</button>
    <button class="toggle-btn" id="btn-cpu">CPU (orange)</button>
  </div>
  <h2>Top Allocations</h2>
  <table class="data-table">
    <thead><tr><th>Location</th><th>Size</th><th>Objects</th><th></th></tr></thead>
    <tbody id="alloc-body"></tbody>
  </table>
  <div class="leak-section" id="leak-section">
    <h2>Suspected Leaks</h2>
    <div id="leak-list"></div>
  </div>`;
}

function buildDashboardScriptTag(
  snapshot: MemoryDashboardSnapshot,
  diff?: MemoryDiffData,
): string {
  const allocJson = JSON.stringify(snapshot.topAllocations);
  const timelineJson = JSON.stringify(snapshot.timeline);
  const leaksJson = JSON.stringify(diff?.suspectedLeaks ?? []);
  const gcCountsJson = JSON.stringify(snapshot.gcCounts);
  return `
    const vscode = acquireVsCodeApi();
    const allocations = ${allocJson};
    const timeline = ${timelineJson};
    const leaks = ${leaksJson};
    const gcCounts = ${gcCountsJson};
    const currentMemory = ${snapshot.currentMemory};
    const peakMemory = ${snapshot.peakMemory};
    const gcObjects = ${snapshot.gcObjects};
    ${PROFILER_JS_UTILS}
    ${buildSummaryScript()}
    ${buildTimelineScript()}
    ${buildToggleScript()}
    ${buildAllocScript()}
    ${buildLeakScript()}
    renderTimeline(); renderAllocs(); renderLeaks();
    window.addEventListener('resize',()=>{renderTimeline();});`;
}

// ── CSS fragments ─────────────────────────────────────────────────────────

function buildDashboardCss(): string {
  return `
    .timeline-container { background:var(--prof-surface); border:1px solid var(--prof-border);
      border-radius:8px; padding:12px; margin-bottom:20px; position:relative; }
    .timeline-canvas { width:100%; height:200px; display:block; }
    .crosshair-label { position:absolute; display:none; background:var(--prof-surface);
      border:1px solid var(--prof-border); border-radius:4px; padding:4px 8px;
      font-size:11px; font-family:'JetBrains Mono',monospace; pointer-events:none;
      white-space:nowrap; z-index:5; }
    .toggle-row { display:flex; gap:8px; margin-bottom:16px; }
    .toggle-btn { background:var(--prof-surface); border:1px solid var(--prof-border);
      border-radius:6px; padding:6px 14px; color:var(--prof-text); font-size:12px;
      font-family:'Space Grotesk',-apple-system,sans-serif; cursor:pointer;
      transition:border-color 120ms ease, color 120ms ease; }
    .toggle-btn:hover { border-color:var(--prof-mem-hot); color:var(--prof-mem-hot); }
    .toggle-btn.active-mem { border-color:var(--prof-mem-critical); color:var(--prof-mem-critical); }
    .toggle-btn.active-cpu { border-color:var(--prof-critical); color:var(--prof-critical); }
    .bar.mem-critical { background:var(--prof-mem-critical); }
    .bar.mem-hot { background:var(--prof-mem-hot); }
    .bar.mem-warm { background:#8b5cf6; }
    .bar.mem-base { background:#7c3aed; }
    .bar.critical { background:var(--prof-critical); }
    .bar.hot { background:var(--prof-hot); }
    .bar.warm { background:var(--prof-warm); }
    .bar.cool { background:var(--prof-cool); }
    .leak-section { margin-top:20px; }
    .leak-row { display:flex; align-items:center; gap:10px; padding:10px 12px;
      background:var(--prof-surface); border:1px solid var(--prof-border);
      border-radius:6px; margin-bottom:8px; cursor:pointer;
      transition:border-color 120ms ease; }
    .leak-row:hover { border-color:var(--prof-mem-leak); }
    .badge { font-size:10px; font-weight:600; text-transform:uppercase;
      letter-spacing:0.06em; padding:2px 8px; border-radius:4px; flex-shrink:0; }
    .badge.definite { background:rgba(248,113,113,0.2); color:#f87171; border:1px solid #f87171; }
    .badge.high { background:rgba(248,113,113,0.1); color:#f87171; border:1px dashed #f87171; }
    .badge.medium { background:rgba(251,191,36,0.15); color:#fbbf24; border:1px solid #fbbf24; }
    .badge.low { background:rgba(136,146,164,0.15); color:#8892a4; border:1px solid #8892a4; }
    .leak-info { flex:1; font-size:13px; font-family:'JetBrains Mono',monospace; }
    .leak-info .leak-file { color:var(--prof-text-secondary); font-size:12px; }
    .leak-info .leak-growth { color:var(--prof-mem-leak); font-weight:500; }
    .leak-info .leak-reason { font-family:'Space Grotesk',-apple-system,sans-serif;
      font-size:11px; color:var(--prof-text-secondary); margin-top:2px; }
    .empty-state { color:var(--prof-text-secondary); font-size:13px;
      padding:16px; text-align:center; }`;
}

function buildSummaryCardsHtml(): string {
  return `
  <div class="summary-cards">
    <div class="card"><div class="label">Current Memory</div>
      <div class="value purple" id="current-mem">0</div></div>
    <div class="card"><div class="label">Peak Memory</div>
      <div class="value purple" id="peak-mem">0</div></div>
    <div class="card"><div class="label">GC Objects</div>
      <div class="value" id="gc-objects">0</div></div>
    <div class="card"><div class="label">GC Counts</div>
      <div class="value" id="gc-counts">\u2014</div></div>
  </div>`;
}

// ── Dashboard script fragments ────────────────────────────────────────────

function buildSummaryScript(): string {
  return `
    document.getElementById('current-mem').textContent = formatBytes(currentMemory);
    document.getElementById('peak-mem').textContent = formatBytes(peakMemory);
    document.getElementById('gc-objects').textContent = gcObjects>=1000?(gcObjects/1000).toFixed(1)+'K':String(gcObjects);
    document.getElementById('gc-counts').textContent = gcCounts.length>0?gcCounts.join(' / '):'\\u2014';`;
}

function buildTimelineScript(): string {
  return [buildTimelineSetupScript(), buildTimelineDrawScript(), buildTimelineCrosshairScript()].join("\n");
}

function buildTimelineSetupScript(): string {
  return `
    const tlCanvas = document.getElementById('timeline-canvas');
    const tlCtx = tlCanvas.getContext('2d');
    const chLabel = document.getElementById('crosshair-label');
    function drawBezierLine(pts, xOf, yOf, key) {
      tlCtx.beginPath(); tlCtx.moveTo(xOf(pts[0].timestamp),yOf(pts[0][key]));
      for(let i=1;i<pts.length;i++){
        const px=xOf(pts[i-1].timestamp),py=yOf(pts[i-1][key]);
        const cx=xOf(pts[i].timestamp),cy=yOf(pts[i][key]);
        tlCtx.bezierCurveTo((px+cx)/2,py,(px+cx)/2,cy,cx,cy);
      }
    }`;
}

function buildTimelineDrawScript(): string {
  return `
    function renderTimeline() {
      const ctr=tlCanvas.parentElement, w=ctr.clientWidth-24, h=200;
      tlCanvas.width=w*devicePixelRatio; tlCanvas.height=h*devicePixelRatio;
      tlCanvas.style.width=w+'px'; tlCanvas.style.height=h+'px';
      tlCtx.setTransform(devicePixelRatio,0,0,devicePixelRatio,0,0);
      if(timeline.length<2){
        tlCtx.fillStyle='#8892a4';tlCtx.font='13px "Space Grotesk",-apple-system,sans-serif';
        tlCtx.textAlign='center';tlCtx.textBaseline='middle';
        tlCtx.fillText('Take multiple snapshots to see the timeline',w/2,h/2); return;
      }
      const pL=60,pR=16,pT=16,pB=32,pW=w-pL-pR,pH=h-pT-pB;
      const t0=timeline[0].timestamp,t1=timeline[timeline.length-1].timestamp,tR=t1-t0||1;
      let mMax=0; for(const p of timeline){if(p.currentMemory>mMax)mMax=p.currentMemory;if(p.peakMemory>mMax)mMax=p.peakMemory;}
      if(mMax===0)mMax=1;
      function xOf(t){return pL+((t-t0)/tR)*pW;} function yOf(m){return pT+pH-(m/mMax)*pH;}
      drawTimelineGrid(pL,pT,pW,pH,mMax,xOf,t0);drawTimelineArea(xOf,yOf,pT,pH);
      drawTimelineLines(xOf,yOf);drawTimelinePoints(xOf,yOf);detectLeakGrowth(pL,pW,pT);
      tlCanvas._geo={pL,pR,pT,pB,pW,pH,t0,t1,tR,mMax,xOf,yOf};
    }
    function drawTimelineGrid(pL,pT,pW,pH,mMax,xOf,t0){
      tlCtx.strokeStyle='#1a1f2e';tlCtx.lineWidth=1;
      for(let i=0;i<=4;i++){
        const y=pT+(pH/4)*i;tlCtx.beginPath();tlCtx.moveTo(pL,y);tlCtx.lineTo(pL+pW,y);tlCtx.stroke();
        tlCtx.fillStyle='#8892a4';tlCtx.font='10px "JetBrains Mono",monospace';
        tlCtx.textAlign='right';tlCtx.textBaseline='middle';tlCtx.fillText(formatBytes(mMax*(1-i/4)),pL-6,y);
      }
      tlCtx.textAlign='center';tlCtx.textBaseline='top';
      const step=Math.max(1,Math.floor(timeline.length/6));
      for(let i=0;i<timeline.length;i+=step){
        const p=timeline[i],el=p.timestamp-t0;
        tlCtx.fillText(el<60?el.toFixed(0)+'s':(el/60).toFixed(1)+'m',xOf(p.timestamp),pT+pH+6);
      }
    }
    function drawTimelineArea(xOf,yOf,pT,pH){
      drawBezierLine(timeline,xOf,yOf,'currentMemory');
      tlCtx.lineTo(xOf(timeline[timeline.length-1].timestamp),pT+pH);
      tlCtx.lineTo(xOf(timeline[0].timestamp),pT+pH);tlCtx.closePath();
      const grad=tlCtx.createLinearGradient(0,pT,0,pT+pH);
      grad.addColorStop(0,'#c084fc30');grad.addColorStop(1,'#c084fc05');
      tlCtx.fillStyle=grad;tlCtx.fill();
    }
    function drawTimelineLines(xOf,yOf){
      drawBezierLine(timeline,xOf,yOf,'currentMemory');
      tlCtx.strokeStyle='#c084fc';tlCtx.lineWidth=2;tlCtx.stroke();
      tlCtx.beginPath();tlCtx.setLineDash([4,4]);
      for(let i=0;i<timeline.length;i++) tlCtx.lineTo(xOf(timeline[i].timestamp),yOf(timeline[i].peakMemory));
      tlCtx.strokeStyle='#a78bfa60';tlCtx.lineWidth=1;tlCtx.stroke();tlCtx.setLineDash([]);
    }
    function drawTimelinePoints(xOf,yOf){
      for(const p of timeline){tlCtx.beginPath();tlCtx.arc(xOf(p.timestamp),yOf(p.currentMemory),3,0,Math.PI*2);tlCtx.fillStyle='#c084fc';tlCtx.fill();}
    }
    function detectLeakGrowth(pL,pW,pT){
      if(timeline.length<3) return;
      let gc=0;for(let i=1;i<timeline.length;i++)if(timeline[i].currentMemory>timeline[i-1].currentMemory)gc++;
      if(gc>=timeline.length-1){tlCtx.fillStyle='#f87171';tlCtx.font='600 11px "Space Grotesk",-apple-system,sans-serif';
        tlCtx.textAlign='right';tlCtx.textBaseline='top';tlCtx.fillText('STEADY GROWTH \\u2014 possible leak',pL+pW,pT+4);}
    }`;
}

function buildTimelineCrosshairScript(): string {
  return `
    tlCanvas.addEventListener('mousemove',(e)=>{
      const g=tlCanvas._geo;if(!g||timeline.length<2)return;
      const r=tlCanvas.getBoundingClientRect(),mx=e.clientX-r.left;
      let ci=0,cd=Infinity;
      for(let i=0;i<timeline.length;i++){const d=Math.abs(g.xOf(timeline[i].timestamp)-mx);if(d<cd){cd=d;ci=i;}}
      const p=timeline[ci];chLabel.style.display='block';
      chLabel.style.left=(mx+12)+'px';chLabel.style.top='12px';
      chLabel.textContent=formatBytes(p.currentMemory)+' (peak: '+formatBytes(p.peakMemory)+')';
    });
    tlCanvas.addEventListener('mouseleave',()=>{chLabel.style.display='none';});`;
}

function buildToggleScript(): string {
  return `
    let heatMode='memory';
    document.getElementById('btn-mem').addEventListener('click',()=>{
      heatMode='memory';
      document.getElementById('btn-mem').className='toggle-btn active-mem';
      document.getElementById('btn-cpu').className='toggle-btn';
      renderAllocs();
    });
    document.getElementById('btn-cpu').addEventListener('click',()=>{
      heatMode='cpu';
      document.getElementById('btn-cpu').className='toggle-btn active-cpu';
      document.getElementById('btn-mem').className='toggle-btn';
      renderAllocs();
    });`;
}

function buildAllocScript(): string {
  return `
    function memBarCls(sz,mx){const r=sz/Math.max(mx,1);if(r>=.5)return'mem-critical';if(r>=.2)return'mem-hot';if(r>=.05)return'mem-warm';return'mem-base';}
    function cpuBarCls(i,t){const r=(t-i)/t;if(r>=.8)return'critical';if(r>=.5)return'hot';if(r>=.2)return'warm';return'cool';}
    function renderAllocs(){
      const body=document.getElementById('alloc-body');body.innerHTML='';
      if(!allocations.length){body.innerHTML='<tr><td colspan="4" class="empty-state">No allocations above threshold</td></tr>';return;}
      const mx=allocations[0].size;
      for(let i=0;i<allocations.length;i++){
        const a=allocations[i],tr=document.createElement('tr');
        tr.onclick=()=>vscode.postMessage({type:'navigateToSource',file:a.file,line:a.line});
        const bw=Math.max(4,(a.size/mx)*140);
        const bc=heatMode==='memory'?memBarCls(a.size,mx):cpuBarCls(i,allocations.length);
        tr.innerHTML='<td style="color:#8892a4;font-size:12px">'+escapeHtml(basename(a.file))+':'+a.line+'</td>'
          +'<td style="font-weight:500">'+formatBytes(a.size)+'</td><td>'+a.count+'</td>'
          +'<td class="bar-cell"><div class="bar '+bc+'" style="width:'+bw+'px"></div></td>';
        body.appendChild(tr);
      }
    }`;
}

function buildLeakScript(): string {
  return `
    function renderLeaks(){
      const list=document.getElementById('leak-list');
      if(!leaks.length){list.innerHTML='<div class="empty-state">No suspected leaks detected. Take more snapshots to enable diff analysis.</div>';return;}
      list.innerHTML='';
      for(const lk of leaks){
        const row=document.createElement('div');row.className='leak-row';
        row.onclick=()=>vscode.postMessage({type:'navigateToSource',file:lk.file,line:lk.line});
        row.innerHTML='<span class="badge '+lk.confidence+'">'+lk.confidence+'</span>'
          +'<div class="leak-info"><div class="leak-file">'+escapeHtml(basename(lk.file))+':'+lk.line+'</div>'
          +'<div class="leak-growth">+'+formatBytes(lk.sizeGrowth)+' ('+lk.countGrowth+' objects)</div>'
          +'<div class="leak-reason">'+escapeHtml(lk.reason)+'</div></div>';
        list.appendChild(row);
      }
    }`;
}
