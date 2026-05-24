// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Inline JavaScript for the reference graph webview Canvas 2D renderer.
 *
 * This module generates the <script> content that runs inside the webview.
 * It implements force-directed layout, rendering, and mouse interaction.
 * Separated from memory-graph-html.ts for the 500 LOC limit.
 */

/** Serialized JSON data injected into the graph webview script. */
interface GraphScriptData {
  nodesJson: string;
  edgesJson: string;
  cyclesJson: string;
  retentionPathJson: string;
}

/** Build the inline JS for the graph webview, injecting serialized data. */
export function buildGraphScript(data: GraphScriptData): string {
  return [
    buildScriptGlobals(data),
    buildScriptHelpers(),
    buildScriptSimulation(),
    buildScriptDrawEdges(),
    buildScriptDrawNodes(),
    buildScriptDrawAndHitTest(),
    buildScriptSidebar(),
    buildScriptInteraction(),
    buildScriptBootstrap(),
  ].join("\n");
}

function buildScriptGlobals(data: GraphScriptData): string {
  return `
    var vscode = acquireVsCodeApi();
    var rawNodes = ${data.nodesJson};
    var rawEdges = ${data.edgesJson};
    var cycles = ${data.cyclesJson};
    var retentionPath = ${data.retentionPathJson};

    var cycleNodeIds = new Set();
    for (var ci = 0; ci < cycles.length; ci++) {
      for (var cj = 0; cj < cycles[ci].length; cj++) {
        cycleNodeIds.add(cycles[ci][cj]);
      }
    }

    var adjFrom = new Map();
    var adjTo = new Map();
    for (var ei = 0; ei < rawEdges.length; ei++) {
      var edge = rawEdges[ei];
      if (!adjFrom.has(edge.from)) { adjFrom.set(edge.from, []); }
      adjFrom.get(edge.from).push(edge);
      if (!adjTo.has(edge.to)) { adjTo.set(edge.to, []); }
      adjTo.get(edge.to).push(edge);
    }

    var canvas = document.getElementById('graph-canvas');
    var ctx = canvas.getContext('2d');
    var tooltipEl = document.getElementById('tooltip');
    var searchInput = document.getElementById('search');
    var positions = new Map();
    var velocities = new Map();
    var NODE_BASE_RADIUS = 12;
    var MAX_NODE_RADIUS = 40;
    var REPULSION = 3000;
    var SPRING_K = 0.005;
    var SPRING_REST = 120;
    var DAMPING = 0.85;
    var CENTER_PULL = 0.001;
    var SIMULATION_STEPS = 200;
    var simulationStep = 0;
    var selectedNodeId = null;
    var hoveredNodeId = null;
    var filterText = '';
    var pulsePhase = 0;
    var draggedNode = null;
    var isDragging = false;`;
}

function buildScriptHelpers(): string {
  return `
    function nodeRadius(size) {
      return Math.min(MAX_NODE_RADIUS, NODE_BASE_RADIUS + Math.sqrt(size / 1024) * 2);
    }
    function nodeColor(node) {
      if (node.isTarget) { return '#c084fc'; }
      if (cycleNodeIds.has(node.id)) { return '#f87171'; }
      if (node.size === 0) { return '#34d399'; }
      if (node.depth <= 1) { return '#a78bfa'; }
      return '#7c8db5';
    }
    function formatBytes(bytes) {
      if (bytes >= 1073741824) { return (bytes / 1073741824).toFixed(1) + ' GB'; }
      if (bytes >= 1048576) { return (bytes / 1048576).toFixed(1) + ' MB'; }
      if (bytes >= 1024) { return (bytes / 1024).toFixed(1) + ' KB'; }
      return bytes + ' B';
    }
    function isNodeVisible(node) {
      if (filterText === '') { return true; }
      return node.type.toLowerCase().includes(filterText);
    }
    function escapeHtml(text) {
      var d = document.createElement('div');
      d.appendChild(document.createTextNode(text));
      return d.innerHTML;
    }
    function findNode(id) {
      for (var i = 0; i < rawNodes.length; i++) {
        if (rawNodes[i].id === id) { return rawNodes[i]; }
      }
      return undefined;
    }`;
}

function buildScriptInitPositions(): string {
  return `
    function initPositions() {
      var cX = canvas.width / 2;
      var cY = canvas.height / 2;
      var r = Math.min(cX, cY) * 0.6;
      for (var i = 0; i < rawNodes.length; i++) {
        var a = (2 * Math.PI * i) / rawNodes.length;
        positions.set(rawNodes[i].id, { x: cX + r * Math.cos(a), y: cY + r * Math.sin(a) });
        velocities.set(rawNodes[i].id, { x: 0, y: 0 });
      }
    }`;
}

function buildScriptSimulation(): string {
  return `${buildScriptInitPositions()}${buildScriptSimulateForces()}`;
}

function buildScriptSimulateForces(): string {
  return `
    function simulateForces() {
      if (simulationStep >= SIMULATION_STEPS) { return; }
      simulationStep++;
      var cX = canvas.width / 2, cY = canvas.height / 2;
      for (var i = 0; i < rawNodes.length; i++) {
        var pA = positions.get(rawNodes[i].id);
        if (!pA) { continue; }
        for (var j = i + 1; j < rawNodes.length; j++) {
          var pB = positions.get(rawNodes[j].id);
          if (!pB) { continue; }
          var dx = pA.x - pB.x;
          var dy = pA.y - pB.y;
          var dist = Math.sqrt(Math.max(dx * dx + dy * dy, 1));
          var f = REPULSION / (dist * dist);
          var fx = (dx / dist) * f;
          var fy = (dy / dist) * f;
          var vA = velocities.get(rawNodes[i].id);
          var vB = velocities.get(rawNodes[j].id);
          if (vA) { vA.x += fx; vA.y += fy; }
          if (vB) { vB.x -= fx; vB.y -= fy; }
        }
      }
      for (var ei = 0; ei < rawEdges.length; ei++) {
        var e = rawEdges[ei];
        var pFrom = positions.get(e.from);
        var pTo = positions.get(e.to);
        if (!pFrom || !pTo) { continue; }
        var dx = pTo.x - pFrom.x;
        var dy = pTo.y - pFrom.y;
        var dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < 0.01) { continue; }
        var disp = dist - SPRING_REST;
        var f = SPRING_K * disp;
        var fx = (dx / dist) * f;
        var fy = (dy / dist) * f;
        var vF = velocities.get(e.from);
        var vT = velocities.get(e.to);
        if (vF) { vF.x += fx; vF.y += fy; }
        if (vT) { vT.x -= fx; vT.y -= fy; }
      }
      for (var ni = 0; ni < rawNodes.length; ni++) {
        var n = rawNodes[ni];
        var p = positions.get(n.id);
        var v = velocities.get(n.id);
        if (!p || !v) { continue; }
        if (n.id === draggedNode) { continue; }
        v.x += (cX - p.x) * CENTER_PULL;
        v.y += (cY - p.y) * CENTER_PULL;
        v.x *= DAMPING;
        v.y *= DAMPING;
        p.x += v.x;
        p.y += v.y;
        var r = nodeRadius(n.size);
        p.x = Math.max(r, Math.min(canvas.width - r, p.x));
        p.y = Math.max(r, Math.min(canvas.height - r, p.y));
      }
    }`;
}

function buildScriptDrawEdges(): string {
  return `
    function drawEdges() {
      for (var i = 0; i < rawEdges.length; i++) {
        var e = rawEdges[i];
        var pA = positions.get(e.from);
        var pB = positions.get(e.to);
        if (!pA || !pB) { continue; }
        var nA = findNode(e.from);
        var nB = findNode(e.to);
        if (!nA || !nB) { continue; }
        if (!isNodeVisible(nA) || !isNodeVisible(nB)) { continue; }
        var isCycle = cycleNodeIds.has(e.from) && cycleNodeIds.has(e.to);
        var isSel = e.from === selectedNodeId || e.to === selectedNodeId;
        ctx.beginPath();
        ctx.moveTo(pA.x, pA.y);
        ctx.lineTo(pB.x, pB.y);
        if (isCycle) {
          ctx.strokeStyle = 'rgba(248,113,113,' + (0.4 + 0.3 * Math.sin(pulsePhase)) + ')';
          ctx.lineWidth = 2.5;
        } else if (isSel) {
          ctx.strokeStyle = 'rgba(192,132,252,0.6)';
          ctx.lineWidth = 2;
        } else {
          ctx.strokeStyle = 'rgba(136,146,164,0.2)';
          ctx.lineWidth = 1;
        }
        ctx.stroke();
        var dx = pB.x - pA.x;
        var dy = pB.y - pA.y;
        var dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < 1) { continue; }
        var rB = nodeRadius(nB.size);
        var ax = pB.x - (dx / dist) * rB;
        var ay = pB.y - (dy / dist) * rB;
        var ang = Math.atan2(dy, dx);
        ctx.beginPath();
        ctx.moveTo(ax, ay);
        ctx.lineTo(ax - 6 * Math.cos(ang - 0.4), ay - 6 * Math.sin(ang - 0.4));
        ctx.lineTo(ax - 6 * Math.cos(ang + 0.4), ay - 6 * Math.sin(ang + 0.4));
        ctx.closePath();
        ctx.fillStyle = isCycle ? '#f87171' : (isSel ? '#c084fc' : '#8892a4');
        ctx.fill();
        if (e.label && (isSel || rawEdges.length < 30)) {
          ctx.font = '10px "JetBrains Mono", monospace';
          ctx.fillStyle = '#8892a4';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'bottom';
          ctx.fillText(e.label, (pA.x + pB.x) / 2, (pA.y + pB.y) / 2 - 4);
        }
      }
    }

`;
}

function buildScriptDrawNodes(): string {
  return `
    function drawNodes() {
      for (var i = 0; i < rawNodes.length; i++) {
        var n = rawNodes[i];
        if (!isNodeVisible(n)) { continue; }
        var p = positions.get(n.id);
        if (!p) { continue; }
        var r = nodeRadius(n.size);
        var c = nodeColor(n);
        var hov = n.id === hoveredNodeId;
        var sel = n.id === selectedNodeId;
        var inCyc = cycleNodeIds.has(n.id);
        if (inCyc) {
          ctx.beginPath();
          ctx.arc(p.x, p.y, r + 8, 0, 2 * Math.PI);
          ctx.fillStyle = 'rgba(248,113,113,' + (0.15 + 0.1 * Math.sin(pulsePhase)) + ')';
          ctx.fill();
        }
        if (sel) {
          ctx.beginPath();
          ctx.arc(p.x, p.y, r + 4, 0, 2 * Math.PI);
          ctx.strokeStyle = '#c084fc';
          ctx.lineWidth = 2;
          ctx.stroke();
        }
        ctx.beginPath();
        ctx.arc(p.x, p.y, r, 0, 2 * Math.PI);
        ctx.fillStyle = c;
        ctx.globalAlpha = hov ? 1.0 : 0.85;
        ctx.fill();
        ctx.globalAlpha = 1.0;
        ctx.beginPath();
        ctx.arc(p.x, p.y, r, 0, 2 * Math.PI);
        ctx.strokeStyle = hov ? '#f0f2f7' : 'rgba(240,242,247,0.15)';
        ctx.lineWidth = hov ? 2 : 1;
        ctx.stroke();
        var lbl = n.type.length > 10 ? n.type.substring(0, 9) + '\\u2026' : n.type;
        ctx.font = '11px "JetBrains Mono", monospace';
        ctx.fillStyle = '#f0f2f7';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(lbl, p.x, p.y);
      }
    }

`;
}

function buildScriptDrawAndHitTest(): string {
  return `
    function draw() {
      var dpr = window.devicePixelRatio || 1;
      canvas.width = canvas.offsetWidth * dpr;
      canvas.height = canvas.offsetHeight * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, canvas.offsetWidth, canvas.offsetHeight);
      drawEdges();
      drawNodes();
      pulsePhase += 0.05;
    }

    function findNodeAt(x, y) {
      for (var i = rawNodes.length - 1; i >= 0; i--) {
        var n = rawNodes[i];
        if (!isNodeVisible(n)) { continue; }
        var p = positions.get(n.id);
        if (!p) { continue; }
        var r = nodeRadius(n.size);
        var dx = x - p.x;
        var dy = y - p.y;
        if (dx * dx + dy * dy <= r * r) { return n; }
      }
      return null;
    }

`;
}

function buildScriptSidebar(): string {
  return `
    function updateSidebar(node) {
      var content = document.getElementById('sidebar-content');
      var pathContainer = document.getElementById('retention-path');
      if (node === null) {
        content.innerHTML = '<div style="color:var(--prof-text-secondary);font-size:13px;">Click a node to view details.</div>';
        pathContainer.innerHTML = '';
        return;
      }
      var badges = [];
      if (node.isTarget) { badges.push('<span class="badge badge-target">TARGET</span>'); }
      if (cycleNodeIds.has(node.id)) { badges.push('<span class="badge badge-cycle">CYCLE</span>'); }
      var refs = (adjTo.get(node.id) || []).map(function(e) {
        var s = findNode(e.from);
        return s ? (s.type + ' (' + e.label + ')') : ('id:' + e.from);
      });
      var ents = (adjFrom.get(node.id) || []).map(function(e) {
        var t = findNode(e.to);
        return t ? (t.type + ' (' + e.label + ')') : ('id:' + e.to);
      });
      content.innerHTML =
        '<div style="margin-bottom:8px;">' + badges.join(' ') + '</div>' +
        '<div class="detail-row"><div class="detail-label">Type</div>' +
          '<div class="detail-value">' + escapeHtml(node.type) + '</div></div>' +
        '<div class="detail-row"><div class="detail-label">Repr</div>' +
          '<div class="detail-value">' + escapeHtml(node.repr) + '</div></div>' +
        '<div class="detail-row"><div class="detail-label">Retained Size</div>' +
          '<div class="detail-value">' + formatBytes(node.size) + '</div></div>' +
        '<div class="detail-row"><div class="detail-label">Depth</div>' +
          '<div class="detail-value">' + node.depth + '</div></div>' +
        '<div class="detail-row"><div class="detail-label">Referrers (' + refs.length + ')</div>' +
          '<div class="detail-value" style="font-size:12px;">' +
          (refs.length > 0 ? refs.map(function(r) { return escapeHtml(r); }).join('<br>') : 'none') +
          '</div></div>' +
        '<div class="detail-row"><div class="detail-label">Referents (' + ents.length + ')</div>' +
          '<div class="detail-value" style="font-size:12px;">' +
          (ents.length > 0 ? ents.map(function(r) { return escapeHtml(r); }).join('<br>') : 'none') +
          '</div></div>';
      if (retentionPath.length > 0) {
        var ph = '<h2 style="margin-top:16px;">Retention Path</h2>';
        for (var pi = 0; pi < retentionPath.length; pi++) {
          ph += '<div class="path-step">' + escapeHtml(retentionPath[pi]) + '</div>';
          if (pi < retentionPath.length - 1) { ph += '<div class="path-arrow">\\u2193</div>'; }
        }
        pathContainer.innerHTML = ph;
      } else {
        pathContainer.innerHTML = '';
      }
    }`;
}

function buildScriptInteraction(): string {
  return `
    canvas.addEventListener('mousedown', function(evt) {
      var rect = canvas.getBoundingClientRect();
      var node = findNodeAt(evt.clientX - rect.left, evt.clientY - rect.top);
      if (node !== null) {
        draggedNode = node.id;
        isDragging = true;
        selectedNodeId = node.id;
        updateSidebar(node);
      } else {
        selectedNodeId = null;
        updateSidebar(null);
      }
    });
    canvas.addEventListener('mousemove', function(evt) {
      var rect = canvas.getBoundingClientRect();
      var mx = evt.clientX - rect.left;
      var my = evt.clientY - rect.top;
      if (isDragging && draggedNode !== null) {
        var p = positions.get(draggedNode);
        if (p) { p.x = mx; p.y = my; }
        simulationStep = Math.max(0, simulationStep - 5);
        return;
      }
      var node = findNodeAt(mx, my);
      if (node !== null) {
        hoveredNodeId = node.id;
        canvas.style.cursor = 'pointer';
        document.getElementById('tt-type').textContent = node.type;
        document.getElementById('tt-size').textContent =
          formatBytes(node.size) + ' \\u2014 ' + node.repr;
        tooltipEl.style.left = (mx + 16) + 'px';
        tooltipEl.style.top = (my + 16) + 'px';
        tooltipEl.classList.add('visible');
      } else {
        hoveredNodeId = null;
        canvas.style.cursor = 'default';
        tooltipEl.classList.remove('visible');
      }
    });
    canvas.addEventListener('mouseup', function() {
      isDragging = false;
      draggedNode = null;
    });
    canvas.addEventListener('mouseleave', function() {
      isDragging = false;
      draggedNode = null;
      hoveredNodeId = null;
      tooltipEl.classList.remove('visible');
    });
`;
}

function buildScriptBootstrap(): string {
  return `
    canvas.addEventListener('dblclick', function(evt) {
      var rect = canvas.getBoundingClientRect();
      var node = findNodeAt(evt.clientX - rect.left, evt.clientY - rect.top);
      if (node !== null) {
        vscode.postMessage({ type: 'expandNode', nodeId: node.id });
      }
    });
    searchInput.addEventListener('input', function() {
      filterText = searchInput.value.toLowerCase();
    });
    function tick() {
      simulateForces();
      draw();
      requestAnimationFrame(tick);
    }

    if (rawNodes.length === 0) {
      document.querySelector('.main-area').innerHTML =
        '<div class="empty-state">No reference data. Run basilisk.memoryReferences on a Python object.</div>';
    } else {
      initPositions();
      tick();
    }`;
}
