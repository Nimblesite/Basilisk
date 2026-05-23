// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Reference graph webview for Basilisk memory profiling.
 *
 * Renders an interactive force-directed graph of object references using
 * Canvas 2D. Nodes are sized by retained memory, colored by type.
 * Edges show reference relationships with labels (.attr, [key], etc.).
 * Cycles are highlighted in red with animated pulse.
 *
 * All graph data comes from the LSP via `basilisk/memory/references`.
 * This module handles only the client-side visualization.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import {
  PROFILER_CSS_VARS,
  PROFILER_CSS_RESET,
  PROFILER_JS_UTILS,
} from "./profiler-styles";

// ── Types ─────────────────────────────────────────────────────────────────

/** A single node in the reference graph. */
export interface ReferenceGraphNode {
  id: number;
  type: string;
  size: number;
  repr: string;
  depth: number;
  isTarget: boolean;
  retainedSize: number;
}

/** A directed edge between two nodes. */
export interface ReferenceGraphEdge {
  from: number;
  to: number;
  label: string;
}

/** Complete reference graph data from the LSP. */
export interface ReferenceGraphData {
  nodes: ReferenceGraphNode[];
  edges: ReferenceGraphEdge[];
  cycles: number[][];
  retentionPath: string[];
}

// ── State ─────────────────────────────────────────────────────────────────

let referenceGraphPanel: vscode.WebviewPanel | undefined;

// ── Public API ────────────────────────────────────────────────────────────

/**
 * Open or reveal the reference graph webview with the given data.
 * If the panel already exists it is updated in place.
 */
export function openReferenceGraphWebview(data: ReferenceGraphData): void {
  if (referenceGraphPanel !== undefined) {
    referenceGraphPanel.reveal(vscode.ViewColumn.Beside);
  } else {
    referenceGraphPanel = vscode.window.createWebviewPanel(
      "basilisk.referenceGraph",
      "Basilisk Reference Graph",
      vscode.ViewColumn.Beside,
      { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] },
    );
    referenceGraphPanel.onDidDispose(() => { referenceGraphPanel = undefined; });
  }

  referenceGraphPanel.webview.html = buildReferenceGraphHtml(data);

  referenceGraphPanel.webview.onDidReceiveMessage(
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

  Logger.info(
    `Reference graph opened: ${data.nodes.length} nodes, ${data.edges.length} edges, ` +
    `${data.cycles.length} cycles`,
  );
}

/** Dispose the reference graph panel if open. */
export function disposeReferenceGraph(): void {
  if (referenceGraphPanel !== undefined) {
    referenceGraphPanel.dispose();
    referenceGraphPanel = undefined;
  }
}

// ── HTML builder ──────────────────────────────────────────────────────────

function buildReferenceGraphHtml(data: ReferenceGraphData): string {
  const nodesJson = JSON.stringify(data.nodes);
  const edgesJson = JSON.stringify(data.edges);
  const cyclesJson = JSON.stringify(data.cycles);
  const retentionPathJson = JSON.stringify(data.retentionPath);

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Basilisk Reference Graph</title>
  <style>
    ${PROFILER_CSS_VARS}
    ${PROFILER_CSS_RESET}
    body { overflow: hidden; height: 100vh; display: flex; flex-direction: column; }
    ${buildToolbarCss()}
    ${buildCanvasCss()}
    ${buildRetentionPathCss()}
    ${buildTooltipCss()}
    ${buildCycleBannerCss()}
  </style>
</head>
<body>
  ${buildToolbarHtml()}
  <div class="cycle-banner" id="cycle-banner">
    Reference cycle detected \u2014 these objects cannot be garbage collected if any has a __del__ method
  </div>
  <div class="canvas-container">
    <canvas id="graph-canvas"></canvas>
    <div class="tooltip" id="tooltip">
      <div class="tt-type" id="tt-type"></div>
      <div class="tt-size" id="tt-size"></div>
      <div class="tt-repr" id="tt-repr"></div>
    </div>
  </div>
  <div class="retention-path" id="retention-path"></div>
  <script>
    const vscode = acquireVsCodeApi();
    const rawNodes = ${nodesJson};
    const rawEdges = ${edgesJson};
    const cycles = ${cyclesJson};
    const retentionPath = ${retentionPathJson};
    ${PROFILER_JS_UTILS}
    ${buildGraphScript()}
  </script>
</body>
</html>`;
}

// ── CSS fragment builders ─────────────────────────────────────────────────

function buildToolbarCss(): string {
  return `
    .toolbar { display:flex; align-items:center; gap:12px; padding:12px 16px;
      border-bottom:1px solid var(--prof-border); flex-shrink:0; flex-wrap:wrap; }
    .toolbar h1 { font-size:16px; font-weight:600; display:flex; align-items:center;
      gap:8px; margin-right:auto; }
    .toolbar h1 .accent { color:var(--prof-mem-critical); }
    .filter-group { display:flex; align-items:center; gap:6px; }
    .filter-group label { font-size:11px; color:var(--prof-text-secondary);
      text-transform:uppercase; letter-spacing:0.05em; }
    .filter-input { background:var(--prof-surface); border:1px solid var(--prof-border);
      border-radius:4px; color:var(--prof-text); padding:4px 8px; font-size:12px;
      font-family:'JetBrains Mono',monospace; outline:none; width:140px; }
    .filter-input:focus { border-color:var(--prof-mem-critical); }`;
}

function buildCanvasCss(): string {
  return `
    .canvas-container { flex:1; position:relative; overflow:hidden; }
    canvas { display:block; width:100%; height:100%; }`;
}

function buildRetentionPathCss(): string {
  return `
    .retention-path { padding:10px 16px; border-top:1px solid var(--prof-border);
      font-family:'JetBrains Mono',monospace; font-size:12px;
      color:var(--prof-text-secondary); max-height:100px; overflow-y:auto; flex-shrink:0; }
    .retention-path .path-label { font-family:'Space Grotesk',-apple-system,sans-serif;
      font-size:11px; text-transform:uppercase; letter-spacing:0.05em;
      color:var(--prof-text-secondary); margin-bottom:4px; }
    .retention-path .path-line { color:var(--prof-mem-hot); }`;
}

function buildTooltipCss(): string {
  return `
    .tooltip { position:absolute; display:none; background:var(--prof-surface);
      border:1px solid var(--prof-border); border-radius:6px; padding:10px 14px;
      font-size:12px; pointer-events:none; max-width:320px; z-index:10; }
    .tooltip .tt-type { font-weight:600; color:var(--prof-mem-critical); margin-bottom:4px; }
    .tooltip .tt-size { font-family:'JetBrains Mono',monospace; color:var(--prof-text); }
    .tooltip .tt-repr { font-family:'JetBrains Mono',monospace; font-size:11px;
      color:var(--prof-text-secondary); margin-top:4px; word-break:break-all; }`;
}

function buildCycleBannerCss(): string {
  return `
    .cycle-banner { display:none; padding:8px 16px;
      background:rgba(248,113,113,0.1); border-bottom:1px solid var(--prof-mem-leak);
      color:var(--prof-mem-leak); font-size:12px; flex-shrink:0; }`;
}

function buildToolbarHtml(): string {
  return `
  <div class="toolbar">
    <h1><span class="accent">RETENTION</span> GRAPH</h1>
    <div class="filter-group">
      <label>Type</label>
      <input class="filter-input" id="filter-type" placeholder="e.g. DataFrame">
    </div>
    <div class="filter-group">
      <label>Min size</label>
      <input class="filter-input" id="filter-size" placeholder="e.g. 1048576" style="width:100px">
    </div>
    <div class="filter-group">
      <label>Search</label>
      <input class="filter-input" id="filter-repr" placeholder="repr substring">
    </div>
  </div>`;
}

// ── Graph script fragments ────────────────────────────────────────────────

function buildGraphScript(): string {
  return [
    buildGraphInitScript(),
    buildGraphSimScript(),
    buildGraphRenderScript(),
    buildGraphInteractionScript(),
    buildGraphFilterScript(),
  ].join("\n");
}

function buildGraphInitScript(): string {
  return `
    const C = { memCritical:'#c084fc', memHot:'#a78bfa', memLeak:'#f87171',
      info:'#60a5fa', textSec:'#8892a4', border:'#1a1f2e', text:'#f0f2f7' };
    const cycleIds = new Set();
    for (const c of cycles) for (const id of c) cycleIds.add(id);
    if (cycles.length > 0) document.getElementById('cycle-banner').style.display = 'block';
    const pathEl = document.getElementById('retention-path');
    if (retentionPath.length > 0) {
      let h = '<div class="path-label">Retention Path</div>';
      for (const l of retentionPath) h += '<div class="path-line">' + escapeHtml(l) + '</div>';
      pathEl.innerHTML = h;
    } else { pathEl.style.display = 'none'; }
    const nodeMap = new Map();
    let simN = [], simE = [];
    const canvas = document.getElementById('graph-canvas');
    const ctx = canvas.getContext('2d');
    const ttEl = document.getElementById('tooltip');
    let aF = 0, tick = 0;
    function nodeR(n) { return Math.min(50, Math.max(8, Math.log10(Math.max(n.size,1)) * 6)); }
    function nodeCol(n) {
      if (cycleIds.has(n.id)) return C.memLeak;
      if (n.isTarget) return C.memCritical;
      if (n.type.startsWith('module:')) return C.info;
      if (['dict','list','tuple','set','frozenset'].includes(n.type)) return C.textSec;
      return C.memHot;
    }`;
}

function buildGraphSimScript(): string {
  return `
    function initSim() {
      simN=[];simE=[];nodeMap.clear();
      const fT=document.getElementById('filter-type').value.trim().toLowerCase();
      const fS=Number(document.getElementById('filter-size').value)||0;
      const fR=document.getElementById('filter-repr').value.trim().toLowerCase();
      const ids=new Set();
      for(const n of rawNodes){
        if(fT&&!n.type.toLowerCase().includes(fT))continue;
        if(n.size<fS)continue;
        if(fR&&!n.repr.toLowerCase().includes(fR))continue;
        ids.add(n.id);
      }
      const conn=new Set(ids);
      for(const e of rawEdges){if(ids.has(e.from))conn.add(e.to);if(ids.has(e.to))conn.add(e.from);}
      const cx=canvas.width/2/devicePixelRatio,cy=canvas.height/2/devicePixelRatio;
      for(const n of rawNodes){
        if(!conn.has(n.id))continue;
        const sn={...n,x:cx+(Math.random()-.5)*400,y:cy+(Math.random()-.5)*400,vx:0,vy:0,radius:nodeR(n)};
        simN.push(sn);nodeMap.set(n.id,sn);
      }
      for(const e of rawEdges) if(nodeMap.has(e.from)&&nodeMap.has(e.to)) simE.push({...e});
    }
    function resizeCanvas(){
      const c=canvas.parentElement;
      canvas.width=c.clientWidth*devicePixelRatio;canvas.height=c.clientHeight*devicePixelRatio;
      ctx.setTransform(devicePixelRatio,0,0,devicePixelRatio,0,0);
    }
    function physics(){
      const w=canvas.width/devicePixelRatio,h=canvas.height/devicePixelRatio;
      for(let i=0;i<simN.length;i++) for(let j=i+1;j<simN.length;j++){
        const a=simN[i],b=simN[j];let dx=a.x-b.x,dy=a.y-b.y,d=Math.sqrt(dx*dx+dy*dy);
        if(d<1){d=1;dx=Math.random()-.5;dy=Math.random()-.5;}
        const f=8000/(d*d),fx=(dx/d)*f,fy=(dy/d)*f;
        a.vx+=fx;a.vy+=fy;b.vx-=fx;b.vy-=fy;
      }
      for(const e of simE){
        const a=nodeMap.get(e.from),b=nodeMap.get(e.to);if(!a||!b)continue;
        const dx=b.x-a.x,dy=b.y-a.y,d=Math.sqrt(dx*dx+dy*dy);if(d<1)continue;
        const f=d*.005,fx=(dx/d)*f,fy=(dy/d)*f;
        a.vx+=fx;a.vy+=fy;b.vx-=fx;b.vy-=fy;
      }
      const cx=w/2,cy=h/2;
      for(const n of simN){
        n.vx+=(cx-n.x)*.01;n.vy+=(cy-n.y)*.01;n.vx*=.85;n.vy*=.85;
        n.x+=n.vx;n.y+=n.vy;
        const p=n.radius+10;
        if(n.x<p){n.x=p;n.vx=0;}if(n.x>w-p){n.x=w-p;n.vx=0;}
        if(n.y<p){n.y=p;n.vy=0;}if(n.y>h-p){n.y=h-p;n.vy=0;}
      }
      tick++;
    }`;
}

function buildGraphRenderScript(): string {
  return `
    function drawEdges(pulse){
      for(const e of simE){
        const a=nodeMap.get(e.from),b=nodeMap.get(e.to);if(!a||!b)continue;
        const cyc=cycleIds.has(e.from)&&cycleIds.has(e.to);
        ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);
        ctx.strokeStyle=cyc?C.memLeak:C.border;ctx.lineWidth=cyc?2+pulse:1;ctx.stroke();
        const dx=b.x-a.x,dy=b.y-a.y,d=Math.sqrt(dx*dx+dy*dy);if(d<1)continue;
        const ux=dx/d,uy=dy/d,ax=b.x-ux*b.radius,ay=b.y-uy*b.radius;
        ctx.beginPath();ctx.moveTo(ax,ay);
        ctx.lineTo(ax-ux*6+uy*3,ay-uy*6-ux*3);ctx.lineTo(ax-ux*6-uy*3,ay-uy*6+ux*3);
        ctx.closePath();ctx.fillStyle=cyc?C.memLeak:C.textSec;ctx.fill();
        if(e.label){ctx.font='10px "JetBrains Mono",monospace';ctx.fillStyle=C.textSec;
          ctx.textAlign='center';ctx.textBaseline='bottom';ctx.fillText(e.label,(a.x+b.x)/2,(a.y+b.y)/2-4);}
      }
    }
    function drawNodes(pulse){
      for(const n of simN){
        const col=nodeCol(n),cyc=cycleIds.has(n.id);
        if(n.isTarget||cyc){
          const gc=cyc?C.memLeak:C.memCritical,gr=n.radius+6+(cyc?pulse*4:0);
          const g=ctx.createRadialGradient(n.x,n.y,n.radius,n.x,n.y,gr);
          g.addColorStop(0,gc+'60');g.addColorStop(1,gc+'00');
          ctx.beginPath();ctx.arc(n.x,n.y,gr,0,Math.PI*2);ctx.fillStyle=g;ctx.fill();
        }
        ctx.beginPath();ctx.arc(n.x,n.y,n.radius,0,Math.PI*2);
        ctx.fillStyle=col+'30';ctx.fill();ctx.strokeStyle=col;ctx.lineWidth=cyc?2+pulse:1.5;ctx.stroke();
        ctx.font='600 11px "Space Grotesk",-apple-system,sans-serif';
        ctx.fillStyle=C.text;ctx.textAlign='center';ctx.textBaseline='middle';
        const lbl=n.type.length>14?n.type.slice(0,12)+'..':n.type;
        ctx.fillText(lbl,n.x,n.y-4);
        ctx.font='500 10px "JetBrains Mono",monospace';ctx.fillStyle=C.textSec;
        ctx.fillText(formatBytes(n.size),n.x,n.y+8);
      }
    }
    function render(){
      const w=canvas.width/devicePixelRatio,h=canvas.height/devicePixelRatio;
      ctx.clearRect(0,0,w,h);
      const pulse=(Math.sin(tick*.08)+1)/2;
      drawEdges(pulse); drawNodes(pulse); physics();
      aF=requestAnimationFrame(render);
    }`;
}

function buildGraphInteractionScript(): string {
  return `
    function nodeAt(cx,cy){
      const r=canvas.getBoundingClientRect(),mx=cx-r.left,my=cy-r.top;
      for(let i=simN.length-1;i>=0;i--){
        const n=simN[i],dx=mx-n.x,dy=my-n.y;
        if(dx*dx+dy*dy<=n.radius*n.radius) return n;
      } return null;
    }
    canvas.addEventListener('mousemove',(e)=>{
      const n=nodeAt(e.clientX,e.clientY);
      canvas.style.cursor=n?'pointer':'default';
      if(n){
        const r=canvas.getBoundingClientRect();
        ttEl.style.display='block';ttEl.style.left=(e.clientX-r.left+16)+'px';
        ttEl.style.top=(e.clientY-r.top+16)+'px';
        document.getElementById('tt-type').textContent=n.type;
        document.getElementById('tt-size').textContent=
          formatBytes(n.size)+(n.retainedSize>n.size?' (retained: '+formatBytes(n.retainedSize)+')':'');
        document.getElementById('tt-repr').textContent=n.repr;
      } else { ttEl.style.display='none'; }
    });
    canvas.addEventListener('click',(e)=>{
      const n=nodeAt(e.clientX,e.clientY);
      if(n) vscode.postMessage({type:'nodeClicked',nodeId:n.id,nodeType:n.type});
    });`;
}

function buildGraphFilterScript(): string {
  return `
    let fTo=null;
    function onFilter(){if(fTo!==null)clearTimeout(fTo);fTo=setTimeout(()=>{tick=0;initSim();},300);}
    document.getElementById('filter-type').addEventListener('input',onFilter);
    document.getElementById('filter-size').addEventListener('input',onFilter);
    document.getElementById('filter-repr').addEventListener('input',onFilter);
    window.addEventListener('resize',()=>{cancelAnimationFrame(aF);resizeCanvas();initSim();render();});
    resizeCanvas(); initSim(); render();`;
}
