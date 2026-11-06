#!/usr/bin/env node
/* Regression test for the freshness claims on /wintermute (index.html).
 *
 * Run:  node site/tests/wintermute-freshness.test.js
 *
 * It loads index.html, builds a tiny DOM stub from its markup, runs the page's
 * inline <script> in a node VM with a fake clock and a fake fetch, and checks
 * what the page SAYS about freshness. No network, no browser. It does not
 * prove layout or rendering in a real browser; it proves the text and state
 * logic only.
 */
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const assert = require('assert');

const HTML = fs.readFileSync(path.join(__dirname, '../wintermute/index.html'), 'utf8');
/* The baked-in snapshot has no recorded observation instant, so the test clock is fixed on its own and never derived from the page. */
const TEST_NOW = Date.parse('2026-11-05T12:00:00Z');
const SNAP_DAY = 'Oct 23, 2026';
const SNAP_PHRASE = 'historical snapshot from ' + SNAP_DAY + ' (exact time unavailable)';
const CLOCK_TIME = /\d{1,2}:\d{2}/;

/* ---------- tiny DOM ---------- */
const VOID = new Set(['img', 'hr', 'br', 'meta', 'link', 'input']);
function decode(s) {
  return s.replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/&#(\d+);/g, (_, d) => String.fromCharCode(+d))
    .replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}
class Text { constructor(d) { this.nodeType = 3; this.data = d; this.parentNode = null; } get textContent() { return this.data; } }
class El {
  constructor(tag) {
    this.nodeType = 1; this.tagName = tag.toLowerCase(); this.attrs = {}; this.children = []; this.parentNode = null;
    const self = this; const cls = () => (self.attrs['class'] || '').split(/\s+/).filter(Boolean);
    this.classList = {
      add(c) { const l = cls(); if (!l.includes(c)) l.push(c); self.attrs['class'] = l.join(' '); },
      remove(c) { self.attrs['class'] = cls().filter(x => x !== c).join(' '); },
      contains(c) { return cls().includes(c); },
      toggle(c, force) { const on = force === undefined ? !cls().includes(c) : !!force; on ? this.add(c) : this.remove(c); return on; }
    };
    this.offsetWidth = 0;
  }
  get className() { return this.attrs['class'] || ''; }
  set className(v) { this.attrs['class'] = v; }
  get firstChild() { return this.children[0] || null; }
  get textContent() { return this.children.map(c => c.textContent).join(''); }
  set textContent(v) { this.children.forEach(c => { c.parentNode = null; }); this.children = []; if (v !== '') this.appendChild(new Text(String(v))); }
  getAttribute(k) { return Object.prototype.hasOwnProperty.call(this.attrs, k) ? this.attrs[k] : null; }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  hasAttribute(k) { return Object.prototype.hasOwnProperty.call(this.attrs, k); }
  appendChild(c) { c.parentNode = this; this.children.push(c); return c; }
  removeChild(c) { const i = this.children.indexOf(c); if (i >= 0) this.children.splice(i, 1); c.parentNode = null; return c; }
  querySelectorAll(sel) { const out = []; walk(this, e => { if (matchSel(e, sel, this)) out.push(e); }); return out; }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}
function walk(root, fn) { root.children.forEach(c => { if (c.nodeType === 1) { fn(c); walk(c, fn); } }); }
function parseCompound(s) {
  const m = { tag: null, cls: [], attrs: [] };
  s.replace(/^[\w-]+/, t => { m.tag = t; return ''; });
  const re = /\.([\w-]+)|\[([\w-]+)(?:="([^"]*)")?\]/g; let x;
  while ((x = re.exec(s))) { if (x[1]) m.cls.push(x[1]); else m.attrs.push([x[2], x[3]]); }
  return m;
}
function matchCompound(e, c) {
  if (c.tag && e.tagName !== c.tag) return false;
  if (!c.cls.every(k => e.classList.contains(k))) return false;
  return c.attrs.every(([k, v]) => e.hasAttribute(k) && (v === undefined || e.getAttribute(k) === v));
}
function matchSel(e, sel, scope) {
  return sel.split(',').some(part => {
    const comps = part.trim().split(/\s+/).map(parseCompound);
    if (!matchCompound(e, comps[comps.length - 1])) return false;
    let node = e.parentNode, i = comps.length - 2;
    while (i >= 0 && node && node !== scope.parentNode) {
      if (matchCompound(node, comps[i])) i--;
      node = node.parentNode;
    }
    return i < 0;
  });
}
function parseBody(html) {
  const bodyOpen = html.match(/<body([^>]*)>/);
  const inner = html.slice(html.indexOf(bodyOpen[0]) + bodyOpen[0].length, html.lastIndexOf('</body>'))
    .replace(/<script[\s\S]*?<\/script>/g, '').replace(/<!--[\s\S]*?-->/g, '');
  const body = new El('body');
  parseAttrs(bodyOpen[1], body);
  const stack = [body];
  const re = /<(\/?)([a-zA-Z][\w-]*)([^>]*?)(\/?)>|([^<]+)/g; let m;
  while ((m = re.exec(inner))) {
    const top = stack[stack.length - 1];
    if (m[5] !== undefined) { top.appendChild(new Text(decode(m[5]))); continue; }
    const name = m[2].toLowerCase();
    if (m[1]) {
      for (let i = stack.length - 1; i > 0; i--) if (stack[i].tagName === name) { stack.length = i; break; }
    } else {
      const el = new El(name); parseAttrs(m[3], el); top.appendChild(el);
      if (!VOID.has(name) && !m[4]) stack.push(el);
    }
  }
  return body;
}
function parseAttrs(s, el) {
  const re = /([\w:-]+)(?:="([^"]*)")?/g; let a;
  while ((a = re.exec(s))) el.setAttribute(a[1], a[2] === undefined ? '' : decode(a[2]));
}

/* ---------- page harness ---------- */
function loadPage(opts) {
  opts = opts || {};
  const clock = { now: opts.now === undefined ? TEST_NOW : opts.now };
  const RealDate = Date;
  class FakeDate extends RealDate {
    constructor(...a) { if (a.length === 0) super(clock.now); else super(...a); }
    static now() { return clock.now; }
  }
  const body = parseBody(HTML);
  const document = {
    body,
    documentElement: new El('html'),
    getElementById(id) { let f = null; walk(body, e => { if (!f && e.getAttribute('id') === id) f = e; }); return f; },
    querySelectorAll: s => body.querySelectorAll(s),
    querySelector: s => body.querySelector(s),
    createElement: t => new El(t),
    createElementNS: (ns, t) => new El(t)
  };
  const timers = [];
  const page = {
    document, clock, body, fetchCalls: 0,
    fetchMode: opts.fetchMode,                 /* function () => promise, or undefined = no fetch at all */
    bar() { return document.getElementById('bar-text').textContent; },
    asOfText() { return document.querySelector('span.live[data-k="as_of"]').textContent; },
    texts(k) { return document.querySelectorAll('span.live[data-k="' + k + '"]').map(e => e.textContent); },
    landedMetas() { return document.querySelectorAll('.live-list li[data-t] .meta').map(e => e.textContent); },
    async flush() { for (let i = 0; i < 5; i++) await new Promise(r => setImmediate(r)); },
    async advance(ms) {
      const end = clock.now + ms;
      for (;;) {
        let next = null;
        timers.forEach(t => { if (next === null || t.at < next.at) next = t; });
        if (!next || next.at > end) break;
        clock.now = next.at; next.at += next.ms; next.fn(); await page.flush();
      }
      clock.now = end; await page.flush();
    }
  };
  const sandbox = {
    document, Date: FakeDate, console,
    setInterval(fn, ms) { timers.push({ fn, ms, at: clock.now + ms }); return timers.length; },
    setTimeout() { return 0; }
  };
  if (opts.fetchMode) sandbox.fetch = function () { page.fetchCalls++; return page.fetchMode(); };
  sandbox.window = sandbox;
  const src = HTML.match(/<script>([\s\S]*?)<\/script>/)[1];
  page.boot = async function () { vm.runInNewContext(src, sandbox); await page.flush(); return page; };
  return page;
}
const iso = ms => new Date(ms).toISOString();
const okJson = j => () => Promise.resolve({ ok: true, json: () => Promise.resolve(j) });
const pending = () => new Promise(() => {});
function feed(page, over, ageMs) {
  const j = Object.assign({ as_of: iso(page.clock.now - (ageMs === undefined ? 60000 : ageMs)), runs_open: 7, queue: 3,
    summa_lookups_7d_hits: 1500, summa_lookups_7d_misses: 130 }, over || {});
  return j;
}
const NOT_LIVE_WORDS = /stale|unavailable|paused|snapshot/i;
function assertNotLive(page, why) {
  assert.ok(!/^live ·/i.test(page.bar()), why + ': status bar must not start with "live ·", got ' + JSON.stringify(page.bar()));
  assert.ok(NOT_LIVE_WORDS.test(page.bar()), why + ': status bar must say stale/unavailable/snapshot, got ' + JSON.stringify(page.bar()));
  assert.ok(page.body.classList.contains('not-live'), why + ': body must carry the not-live class (no pulsing dots)');
}
function assertLive(page, why) {
  assert.ok(/^live ·/i.test(page.bar()), why + ': status bar must start with "live ·", got ' + JSON.stringify(page.bar()));
  assert.ok(!/stale|unavailable|paused|snapshot/i.test(page.bar()), why + ': live bar must not carry a warning word: ' + page.bar());
  assert.ok(!page.body.classList.contains('not-live'), why + ': body must not be not-live');
}
const ABS_RE = /^[A-Z][a-z]{2} \d{1,2}, \d{4}, \d{1,2}:\d{2} [AP]M UTC/;
const absOf = ms => new Date(ms).toLocaleString('en-US', { timeZone: 'UTC', month: 'short', day: 'numeric', year: 'numeric', hour: 'numeric', minute: '2-digit', timeZoneName: 'short' });

/* ---------- tests ---------- */
const tests = [];
const test = (name, fn) => tests.push([name, fn]);

test('static markup (no JS): historical snapshot, exact observation time unknown, no fake freshness', async () => {
  const page = loadPage({});                            /* script is NOT run */
  assert.ok(!page.body.getAttribute('data-asof'), 'snapshot observation instant must be unknown (data-asof empty or missing), got ' + JSON.stringify(page.body.getAttribute('data-asof')));
  const bar = page.bar();
  assert.ok(bar.includes(SNAP_PHRASE), 'bar says historical snapshot, date only, exact time unavailable, got ' + JSON.stringify(bar));
  assert.ok(!CLOCK_TIME.test(bar), 'bar must not invent a time of day: ' + bar);
  assert.ok(!/^live ·/i.test(bar), 'bar must not start with "live ·": ' + bar);
  assert.ok(page.body.classList.contains('not-live'), 'body starts not-live (dots do not pulse as live)');
  const asof = page.asOfText();
  assert.ok(asof.includes(SNAP_DAY) && /historical snapshot/i.test(asof) && /exact time unavailable/i.test(asof), 'as_of span: date + historical snapshot + time unavailable, got ' + JSON.stringify(asof));
  assert.ok(!CLOCK_TIME.test(asof), 'as_of span must not invent a time of day: ' + asof);
  const text = page.body.textContent;
  assert.ok(!/\bmin ago\b|just now/.test(text), 'no "min ago"/"just now" anywhere in static text');
  const metas = page.landedMetas();
  assert.ok(metas.length >= 5, 'landed list present');
  metas.forEach(m => assert.ok(/^landed /.test(m) && ABS_RE.test(m.slice(7)), 'landed meta absolute, got ' + JSON.stringify(m)));
  /* each landed meta must equal its own data-t instant */
  page.document.querySelectorAll('.live-list li[data-t]').forEach(li => {
    assert.strictEqual(li.querySelector('.meta').textContent, 'landed ' + absOf(Date.parse(li.getAttribute('data-t'))));
  });
});

test('By the numbers wiki count uses the Memory section data-k attributes and snapshot values', async () => {
  const page = loadPage({});
  for (const [k, v] of [['summa_lookups_7d_hits', '1,455'], ['summa_lookups_7d_misses', '129']]) {
    const t = page.texts(k);
    assert.ok(t.length >= 2, k + ' should appear in both Memory and By the numbers, found ' + t.length);
    t.forEach(x => assert.strictEqual(x, v, k + ' snapshot value must match across sections'));
  }
  const sec = HTML.slice(HTML.indexOf('id="by-the-numbers"'));
  const list = sec.slice(0, sec.indexOf('<hr>'));
  assert.ok(list.includes('data-k="summa_lookups_7d_hits"') && list.includes('data-k="summa_lookups_7d_misses"'), 'data-k spans live inside By the numbers');
  assert.ok(!/1,406|128 misses/.test(HTML), 'old unlinked wiki numbers are gone');
});

test('no fetch available: stays a labelled snapshot, never live', async () => {
  const page = await loadPage({ fetchMode: undefined }).boot();
  assertNotLive(page, 'no fetch');
  assert.ok(page.bar().includes(SNAP_PHRASE), 'shows historical snapshot, exact time unavailable: ' + page.bar());
  assert.ok(!CLOCK_TIME.test(page.bar()) && !CLOCK_TIME.test(page.asOfText()), 'no invented time of day: ' + page.bar() + ' / ' + page.asOfText());
  assert.strictEqual(page.texts('runs_open')[0], '4', 'snapshot values kept');
  page.landedMetas().forEach(m => assert.ok(!/min ago|just now/.test(m), 'historical release not minutes ago: ' + m));
  await page.advance(10 * 60000);
  assertNotLive(page, 'no fetch after 10 min');
});

test('fresh feed: live, with the accurate observation time', async () => {
  const page = loadPage({ fetchMode: () => okJson(feed(page_, { spark: { runs_open: [1, 2, 3] }, recently_landed: [{ title: 'x', landed_at: iso(page_.clock.now - 5 * 60000) }] }))() });
  var page_ = page;
  await page.boot();
  assertLive(page, 'fresh');
  const obs = page.clock.now - 60000;
  assert.ok(page.asOfText().startsWith(absOf(obs)), 'as_of shows observation time ' + absOf(obs) + ', got ' + page.asOfText());
  assert.ok(/1 min ago/.test(page.asOfText()), 'age is 1 min: ' + page.asOfText());
  assert.strictEqual(page.texts('runs_open')[0], '7', 'values applied');
  assert.deepStrictEqual(page.landedMetas(), ['landed 5 min ago']);
  assert.ok(page.document.querySelector('.spark[data-s="runs_open"] svg'), 'sparkline still drawn');
});

test('wiki count spans update together from one feed', async () => {
  const page = loadPage({ fetchMode: () => okJson(feed(page_))() }); var page_ = page;
  await page.boot();
  assert.deepStrictEqual(page.texts('summa_lookups_7d_hits'), ['1,500', '1,500']);
  assert.deepStrictEqual(page.texts('summa_lookups_7d_misses'), ['130', '130']);
});

test('stale feed timestamp (> 5 min): says stale, not live, still shows data and its time', async () => {
  const page = loadPage({ fetchMode: () => okJson(feed(page_, {}, 10 * 60000))() }); var page_ = page;
  await page.boot();
  assertNotLive(page, 'stale');
  assert.ok(/stale/i.test(page.bar()), 'bar says stale: ' + page.bar());
  assert.ok(page.asOfText().startsWith(absOf(page.clock.now - 10 * 60000)), 'as_of shows the old observation time: ' + page.asOfText());
  assert.ok(/stale/i.test(page.asOfText()), 'as_of span says stale: ' + page.asOfText());
});

test('time advancing without any fetch turns live into stale', async () => {
  const state = { mode: null };
  const page = loadPage({ fetchMode: () => (state.mode ? state.mode() : okJson(feed(page_))()) }); var page_ = page;
  await page.boot();
  assertLive(page, 'at start');
  state.mode = pending;                        /* every later fetch hangs */
  const obsText = page.asOfText().split(' (')[0];
  await page.advance(3 * 60000);
  assertLive(page, '3 min later (still within 5 min)');
  await page.advance(3 * 60000);               /* 6 min after start; reading is 7 min old */
  assertNotLive(page, '6 min later');
  assert.ok(/stale/i.test(page.bar()), page.bar());
  assert.strictEqual(page.asOfText().split(' (')[0], obsText, 'observation time retained');
  assert.strictEqual(page.texts('runs_open')[0], '7', 'last good data retained');
});

test('invalid, absent, empty or malformed success responses keep last good values and claim no freshness', async () => {
  const cases = {
    'invalid timestamp': p => feed(p, { as_of: 'not-a-date', runs_open: 99 }),
    'absent timestamp': p => { const j = feed(p, { runs_open: 99 }); delete j.as_of; return j; },
    'future timestamp': p => feed(p, { as_of: iso(p.clock.now + 3600e3), runs_open: 99 }),
    'empty object': () => ({}),
    'timestamp only': p => ({ as_of: iso(p.clock.now - 1000) }),
    'array': () => [],
    'null': () => null,
    'string': () => 'oops'
  };
  for (const name of Object.keys(cases)) {
    const state = { next: null };
    const page = loadPage({ fetchMode: () => (state.next ? okJson(state.next())() : okJson(feed(page_))()) }); var page_ = page;
    await page.boot();
    assertLive(page, name + ' / baseline');
    const obsText = page.asOfText().split(' (')[0];
    state.next = () => cases[name](page);
    await page.advance(120000);
    assert.strictEqual(page.texts('runs_open')[0], '7', name + ': values must not be wiped or replaced');
    assert.strictEqual(page.texts('queue')[0], '3', name + ': other values kept');
    assert.strictEqual(page.asOfText().split(' (')[0], obsText, name + ': last observation time retained, not advanced');
    assertNotLive(page, name);
  }
});

test('valid timestamp with a malformed list does not wipe the list', async () => {
  const state = { next: null };
  const page = loadPage({ fetchMode: () => okJson(state.next ? state.next() : feed(page_, { recently_landed: [{ title: 'kept', landed_at: iso(page_.clock.now - 60000) }] }))() }); var page_ = page;
  await page.boot();
  assert.deepStrictEqual(page.landedMetas(), ['landed 1 min ago']);
  state.next = () => feed(page, { recently_landed: 'oops', runs_open: 8 });
  await page.advance(120000);
  assert.strictEqual(page.texts('runs_open')[0], '8', 'valid fields applied');
  assert.strictEqual(page.document.querySelector('ul.live-list[data-k="recently_landed"] strong').textContent, 'kept', 'list not wiped');
});

test('failed fetch (reject, http 500, bad json): says unavailable, retains data and last observation time', async () => {
  const modes = {
    reject: () => Promise.reject(new Error('net')),
    http500: () => Promise.resolve({ ok: false, status: 500, json: () => Promise.resolve({}) }),
    badjson: () => Promise.resolve({ ok: true, json: () => Promise.reject(new SyntaxError('bad')) }),
    throws: () => { throw new Error('sync'); }
  };
  for (const name of Object.keys(modes)) {
    const state = { mode: null };
    const page = loadPage({ fetchMode: () => (state.mode ? state.mode() : okJson(feed(page_))()) }); var page_ = page;
    await page.boot();
    assertLive(page, name + ' / baseline');
    const obsText = page.asOfText().split(' (')[0];
    state.mode = modes[name];
    await page.advance(120000);
    assertNotLive(page, name);
    assert.ok(/unavailable|stale/i.test(page.bar()), name + ': ' + page.bar());
    assert.ok(page.bar().includes(obsText), name + ': last observation time in the bar: ' + page.bar());
    assert.strictEqual(page.asOfText().split(' (')[0], obsText, name + ': observation time retained');
    assert.strictEqual(page.texts('runs_open')[0], '7', name + ': last good data retained');
  }
});

test('first fetch fails: snapshot stays labelled as snapshot, unavailable', async () => {
  const page = loadPage({ fetchMode: () => Promise.reject(new Error('net')) });
  await page.boot();
  assertNotLive(page, 'first fetch fails');
  assert.ok(/unavailable/i.test(page.bar()), page.bar());
  assert.ok(page.bar().includes(SNAP_PHRASE), page.bar());
  assert.ok(!CLOCK_TIME.test(page.bar()) && !CLOCK_TIME.test(page.asOfText()), 'no invented time of day: ' + page.bar() + ' / ' + page.asOfText());
  assert.ok(/historical snapshot/i.test(page.asOfText()), 'as_of span still labelled historical: ' + page.asOfText());
  assert.strictEqual(page.texts('runs_open')[0], '4');
});

test('recovery: a fresh feed after a failure is live again', async () => {
  const state = { mode: () => Promise.reject(new Error('net')) };
  const page = loadPage({ fetchMode: () => state.mode() });
  await page.boot();
  assertNotLive(page, 'down');
  state.mode = () => okJson(feed(page, {}, 30000))();
  await page.advance(120000);
  assertLive(page, 'recovered');
});

(async () => {
  let failed = 0;
  for (const [name, fn] of tests) {
    try { await fn(); console.log('PASS  ' + name); }
    catch (e) { failed++; console.log('FAIL  ' + name + '\n      ' + String(e && e.message || e).split('\n')[0]); }
  }
  console.log(failed ? '\n' + failed + ' of ' + tests.length + ' FAILED' : '\nall ' + tests.length + ' passed');
  process.exit(failed ? 1 : 0);
})();
