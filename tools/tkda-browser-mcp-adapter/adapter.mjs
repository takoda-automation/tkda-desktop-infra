#!/usr/bin/env node
import { createServer } from 'node:http';
import { createHash, randomUUID, timingSafeEqual } from 'node:crypto';
import { readFileSync } from 'node:fs';

const MAX_BODY_BYTES = 1_048_576;
const MAX_SESSIONS = 8;
const DEFAULT_BIND = '127.0.0.1:18090';
const DEFAULT_DAEMON = 'http://127.0.0.1:18087';
const DEFAULT_TIMEOUT_MS = 60_000;

function env(name, fallback = '') {
  const value = process.env[name];
  return value && value.trim() ? value.trim() : fallback;
}

function readSecret(valueName, fileName) {
  const direct = env(valueName);
  if (direct) return direct;
  const file = env(fileName);
  if (!file) throw new Error(`set ${valueName} or ${fileName}`);
  return readFileSync(file, 'utf8').trim();
}

function parseLoopbackUrl(raw, label) {
  const url = new URL(raw);
  if (url.protocol !== 'http:') throw new Error(`${label} must use http:// loopback`);
  if (!['127.0.0.1', 'localhost', '[::1]', '::1'].includes(url.hostname)) {
    throw new Error(`${label} must target loopback`);
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error(`${label} may not contain credentials, query, or fragment`);
  }
  return url.href.replace(/\/$/, '');
}

function parseBind(raw) {
  const index = raw.lastIndexOf(':');
  if (index <= 0) throw new Error('TKDA_BROWSER_MCP_ADAPTER_BIND must be host:port');
  const host = raw.slice(0, index);
  const port = Number(raw.slice(index + 1));
  if (!['127.0.0.1', 'localhost', '::1'].includes(host) || !Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('browser MCP adapter must bind to loopback on a valid port');
  }
  return { host, port };
}

const bind = parseBind(env('TKDA_BROWSER_MCP_ADAPTER_BIND', DEFAULT_BIND));
const daemonBase = parseLoopbackUrl(env('TKDA_LOCAL_CONTROL_URL', DEFAULT_DAEMON), 'TKDA_LOCAL_CONTROL_URL');
const localControlToken = readSecret('TKDA_LOCAL_CONTROL_TOKEN', 'TKDA_LOCAL_CONTROL_TOKEN_FILE');
const workerSecret = readSecret('TKDA_BROWSER_MCP_WORKER_SECRET', 'TKDA_BROWSER_MCP_WORKER_SECRET_FILE');
const executionMode = env('TKDA_BROWSER_MCP_EXECUTION_MODE', 'headless');
if (!['headed', 'headless'].includes(executionMode)) throw new Error('TKDA_BROWSER_MCP_EXECUTION_MODE must be headed or headless');
if (localControlToken.length < 32 || workerSecret.length < 32) throw new Error('browser MCP secrets must be at least 32 characters');

const sessions = new Map();

function sendJson(res, status, body) {
  const payload = Buffer.from(JSON.stringify(body));
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'content-length': String(payload.length),
    'cache-control': 'no-store',
  });
  res.end(payload);
}

function constantTimeEqual(a, b) {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  if (left.length !== right.length) return false;
  return timingSafeEqual(left, right);
}

function authorized(req) {
  const supplied = req.headers['x-server-auth'];
  return typeof supplied === 'string' && constantTimeEqual(workerSecret, supplied);
}

async function readJson(req) {
  const chunks = [];
  let total = 0;
  for await (const chunk of req) {
    total += chunk.length;
    if (total > MAX_BODY_BYTES) throw Object.assign(new Error('request body too large'), { status: 413 });
    chunks.push(chunk);
  }
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}');
  } catch {
    throw Object.assign(new Error('invalid JSON'), { status: 400 });
  }
}

async function daemon(path, options = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), options.timeoutMs ?? DEFAULT_TIMEOUT_MS);
  try {
    const response = await fetch(`${daemonBase}${path}`, {
      method: options.method ?? 'GET',
      headers: {
        authorization: `Bearer ${localControlToken}`,
        ...(options.body === undefined ? {} : { 'content-type': 'application/json' }),
      },
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      signal: controller.signal,
      redirect: 'error',
    });
    const text = await response.text();
    let body;
    try { body = text ? JSON.parse(text) : {}; } catch { body = { error: 'non-JSON daemon response' }; }
    if (!response.ok) {
      const error = new Error(body.error ?? `Takoda daemon returned HTTP ${response.status}`);
      error.status = response.status;
      error.body = body;
      throw error;
    }
    return body;
  } finally {
    clearTimeout(timer);
  }
}

async function waitForRun(runId) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    const snapshot = await daemon(`/v1/runs/${encodeURIComponent(runId)}`);
    if (snapshot.status === 'running') return snapshot;
    if (['cancelled', 'failed', 'timed_out', 'succeeded'].includes(snapshot.status)) {
      throw Object.assign(new Error(`Takoda run became terminal: ${snapshot.status}`), { status: 502 });
    }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  throw Object.assign(new Error('Takoda browser run did not become ready in time'), { status: 504 });
}

async function createRun() {
  const snapshot = await daemon('/v1/runs', {
    method: 'POST',
    body: {
      task_id: `browser-mcp-${randomUUID()}`,
      prompt: 'Interactive browser session controlled through browser_state/browser_act.',
      language: 'typescript',
      browser_engine: 'playwright',
      source_revision: null,
      execution_target: 'local',
      placement_preference: 'desktop',
      execution_mode: executionMode,
      timeout_secs: 7200,
      max_retries: 0,
      ai: { enabled: false, max_planning_steps: 16, max_replans: 0 },
    },
  });
  if (!snapshot.run_id) throw Object.assign(new Error('Takoda daemon did not return run_id'), { status: 502 });
  await waitForRun(snapshot.run_id);
  return snapshot.run_id;
}

async function driver(runId, action, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const response = await daemon(`/v1/runs/${encodeURIComponent(runId)}/driver/wait`, {
    method: 'POST',
    timeoutMs: Math.min(125_000, Math.max(1_000, timeoutMs + 5_000)),
    body: {
      method: 'POST',
      path: '/browser',
      body: action,
      timeout_ms: Math.min(120_000, Math.max(100, timeoutMs)),
    },
  });
  if (Number(response.status) >= 400) {
    throw Object.assign(new Error(response.body?.error ?? 'Takoda browser driver failed'), { status: 502 });
  }
  return response.body ?? {};
}

function domainAllowed(raw, allowedDomains) {
  if (!allowedDomains?.length) return true;
  let host;
  try { host = new URL(raw).hostname.toLowerCase(); } catch { return false; }
  return allowedDomains.some((domain) => {
    const d = String(domain).trim().toLowerCase();
    return d && (host === d || host.endsWith(`.${d}`));
  });
}

function blockerFromSnapshot(snapshot) {
  const s = snapshot?.signals ?? {};
  if (s.captcha) return { type: 'captcha', message: 'CAPTCHA challenge detected; human handling required.' };
  if (s.mfa) return { type: 'mfa', message: 'MFA or one-time-code challenge detected; human handling required.' };
  if (s.payment) return { type: 'payment', message: 'Payment fields detected; human handling required.' };
  if (s.signature) return { type: 'signature', message: 'Electronic signature detected; human handling required.' };
  if (s.legal_attestation) return { type: 'legal_attestation', message: 'Legal attestation detected; explicit user input is required.' };
  return null;
}

function projectSnapshot(session, snapshot, sinceRevision, timedOut = false) {
  const elements = Array.isArray(snapshot?.elements) ? snapshot.elements : [];
  session.elements = elements;
  session.lastSnapshot = snapshot;
  const fields = elements.filter((item) => ['textbox', 'combobox', 'checkbox', 'radio'].includes(item.role));
  const buttons = elements.filter((item) => item.role === 'button');
  const links = elements.filter((item) => item.role === 'link');
  const blocker = blockerFromSnapshot(snapshot);
  session.blocker = blocker;
  return {
    session_id: session.id,
    revision: session.revision,
    ...(sinceRevision === undefined ? {} : { previous_revision: sinceRevision }),
    changed: sinceRevision === undefined ? true : sinceRevision !== session.revision,
    timed_out: timedOut,
    page: {
      url: snapshot?.url ?? '',
      origin: (() => { try { return new URL(snapshot?.url ?? '').origin; } catch { return ''; } })(),
      title: snapshot?.title ?? '',
      load_state: 'load',
      secure_context: String(snapshot?.url ?? '').startsWith('https:'),
    },
    summary: `Page with ${elements.length} interactive control(s).${blocker ? ` Blocker: ${blocker.type}.` : ''}`,
    visible_text: {
      untrusted_content: String(snapshot?.visible_text ?? '').slice(0, 30_000),
      truncated: String(snapshot?.visible_text ?? '').length > 30_000,
    },
    interactive_elements: elements,
    fields,
    buttons,
    links,
    ...(blocker ? { blocker } : {}),
  };
}

async function snapshot(session, maxElements = 200) {
  return await driver(session.runId, { op: 'snapshot', max_elements: Math.min(500, Math.max(1, maxElements)) });
}

function findTarget(session, target = {}) {
  const elements = session.elements ?? [];
  if (target.ref) {
    const found = elements.find((item) => item.ref === target.ref);
    if (!found) throw Object.assign(new Error(`ref ${target.ref} is not in the latest browser_state`), { status: 422, code: 'target_not_found' });
    return found;
  }
  if (target.css_fallback) return { selector: String(target.css_fallback), name: target.name ?? '' };

  const pairs = [
    ['role', target.role],
    ['name', target.name],
    ['label', target.label],
    ['placeholder', target.placeholder],
  ].filter(([, value]) => typeof value === 'string' && value.length);
  if (!pairs.length) throw Object.assign(new Error('target must contain ref or a semantic selector'), { status: 400, code: 'invalid_request' });

  const exact = target.exact === true;
  const matches = elements.filter((item) => pairs.every(([key, expected]) => {
    const actual = String(item[key] ?? '');
    return exact ? actual === expected : actual.toLowerCase().includes(String(expected).toLowerCase());
  }));
  if (!matches.length) throw Object.assign(new Error('no element matched target'), { status: 422, code: 'target_not_found' });
  if (matches.length > 1 && target.nth === undefined) {
    throw Object.assign(new Error(`target matched ${matches.length} elements; refine it or specify nth`), { status: 422, code: 'ambiguous_target' });
  }
  return matches[target.nth ?? 0];
}

function consequential(action, element) {
  if (action.type === 'submit') return true;
  if (action.type !== 'click') return false;
  const label = `${element?.name ?? ''} ${element?.label ?? ''}`;
  return /\b(submit|apply|send|confirm|complete|finish|agree|accept|purchase|pay|sign|register)\b/i.test(label);
}

function digestFor(session, action, element) {
  const detail = `${action.type}:${element?.ref ?? ''}:${element?.name ?? element?.label ?? ''}`;
  return 'sha256:' + createHash('sha256').update([session.id, String(session.revision), detail].join('\0')).digest('hex');
}

function confirmed(req, session, digest) {
  return req.confirmation?.action_digest === digest
    && req.confirmation?.confirmed_revision === session.revision
    && req.confirmation?.user_explicitly_approved === true;
}

async function handleAct(req) {
  const owner = String(req.owner ?? 'anonymous').slice(0, 200);
  let session;
  if (!req.session_id) {
    if (sessions.size >= MAX_SESSIONS) throw Object.assign(new Error('local browser session limit reached'), { status: 429, code: 'too_many_sessions' });
    const start = (req.actions ?? []).find((action) => action.type === 'start');
    if (!start) throw Object.assign(new Error('a new session requires a start action'), { status: 400, code: 'invalid_request' });
    if (start.initial_url && !domainAllowed(start.initial_url, req.allowed_domains)) {
      throw Object.assign(new Error('initial URL is outside the workflow domain allowlist'), { status: 403, code: 'domain_not_allowed' });
    }
    const runId = await createRun();
    session = {
      id: randomUUID().replaceAll('-', '') + randomUUID().replaceAll('-', ''),
      owner,
      runId,
      revision: 0,
      elements: [],
      lastSnapshot: null,
      blocker: null,
      idempotency: new Map(),
    };
    sessions.set(session.id, session);
  } else {
    session = sessions.get(req.session_id);
    if (!session || session.owner !== owner) throw Object.assign(new Error('no such session'), { status: 404, code: 'session_not_found' });
    if (req.request_id && session.idempotency.has(req.request_id)) return session.idempotency.get(req.request_id);
    if (req.expected_revision !== undefined && req.expected_revision !== session.revision) {
      return {
        request_id: req.request_id,
        session_id: session.id,
        revision: session.revision,
        status: 'revision_conflict',
        action_results: [],
        changed: false,
        summary: `Stale expected_revision ${req.expected_revision}; re-observe.`,
      };
    }
  }

  const results = [];
  let changed = false;
  try {
    let latest = await snapshot(session);
    projectSnapshot(session, latest);

    for (let index = 0; index < (req.actions ?? []).length; index += 1) {
      const action = req.actions[index];
      if (action.type === 'start') {
        if (action.initial_url) {
          if (!domainAllowed(action.initial_url, req.allowed_domains)) throw Object.assign(new Error('initial URL is outside the workflow domain allowlist'), { status: 403, code: 'domain_not_allowed' });
          await driver(session.runId, { op: 'navigate', url: action.initial_url });
          changed = true;
        }
        results.push({ index, type: 'start', status: 'completed' });
        continue;
      }

      if (action.type === 'close') {
        await daemon(`/v1/runs/${encodeURIComponent(session.runId)}/cancel`, { method: 'POST', body: {} });
        sessions.delete(session.id);
        results.push({ index, type: 'close', status: 'completed' });
        return {
          request_id: req.request_id,
          session_id: session.id,
          revision: session.revision + 1,
          status: 'completed',
          page: { url: '', origin: '', title: '', load_state: 'closed' },
          action_results: results,
          changed: true,
          summary: 'Browser session closed.',
        };
      }

      latest = await snapshot(session);
      projectSnapshot(session, latest);
      const interaction = new Set(['fill', 'type', 'fill_form', 'click', 'submit', 'select', 'check', 'uncheck', 'press', 'upload']);
      if (session.blocker && interaction.has(action.type)) {
        results.push({ index, type: action.type, status: 'blocked', message: session.blocker.message });
        return {
          request_id: req.request_id,
          session_id: session.id,
          revision: session.revision,
          status: 'blocked',
          action_results: results,
          blocker: session.blocker,
          changed,
          summary: `Blocked: ${session.blocker.message}`,
        };
      }

      if (action.type === 'goto') {
        if (!domainAllowed(action.url, req.allowed_domains)) throw Object.assign(new Error('destination is outside the workflow domain allowlist'), { status: 403, code: 'domain_not_allowed' });
        await driver(session.runId, { op: 'navigate', url: action.url });
        changed = true;
      } else if (action.type === 'fill' || action.type === 'type') {
        const element = findTarget(session, action.target);
        const value = action.value?.literal;
        if (typeof value !== 'string') throw Object.assign(new Error('Takoda adapter currently accepts literal field values only'), { status: 422, code: 'secret_required' });
        await driver(session.runId, { op: 'fill', selector: element.selector, value });
        changed = true;
      } else if (action.type === 'fill_form') {
        for (const field of action.fields ?? []) {
          const element = findTarget(session, field.target);
          const value = field.value?.literal;
          if (typeof value !== 'string') throw Object.assign(new Error('Takoda adapter currently accepts literal field values only'), { status: 422, code: 'secret_required' });
          await driver(session.runId, { op: 'fill', selector: element.selector, value });
        }
        changed = true;
      } else if (action.type === 'click' || action.type === 'submit') {
        const element = findTarget(session, action.target);
        if (consequential(action, element)) {
          const digest = digestFor(session, action, element);
          if (!confirmed(req, session, digest)) {
            const pending = {
              description: `${action.type === 'submit' ? 'Submit' : 'Click'} "${String(element.name ?? element.label ?? element.ref).slice(0, 120)}"`,
              target_ref: element.ref ?? '',
              page_url: latest.url ?? '',
              revision: session.revision,
              action_digest: digest,
              consequences: ['May submit a form or trigger an irreversible action'],
            };
            results.push({ index, type: action.type, status: 'blocked', message: 'needs confirmation' });
            return {
              request_id: req.request_id,
              session_id: session.id,
              revision: session.revision,
              status: 'needs_confirmation',
              action_results: results,
              pending_action: pending,
              changed,
              summary: 'Consequential action requires explicit confirmation.',
            };
          }
        }
        await driver(session.runId, { op: 'click', selector: element.selector });
        changed = true;
      } else if (action.type === 'select') {
        const element = findTarget(session, action.target);
        await driver(session.runId, { op: 'select', selector: element.selector, option: action.option });
        changed = true;
      } else if (action.type === 'check' || action.type === 'uncheck') {
        const element = findTarget(session, action.target);
        await driver(session.runId, { op: action.type, selector: element.selector });
        changed = true;
      } else if (action.type === 'press') {
        const element = action.target ? findTarget(session, action.target) : null;
        await driver(session.runId, { op: 'press', ...(element ? { selector: element.selector } : {}), key: action.key });
        changed = true;
      } else if (action.type === 'scroll') {
        const element = action.target ? findTarget(session, action.target) : null;
        await driver(session.runId, { op: 'scroll', ...(element ? { selector: element.selector } : {}), delta_x: action.delta_x ?? 0, delta_y: action.delta_y ?? 600 });
        changed = true;
      } else if (action.type === 'screenshot') {
        const shot = await driver(session.runId, { op: 'screenshot' });
        results.push({ index, type: action.type, status: 'completed' });
        session.lastScreenshot = shot;
        continue;
      } else if (action.type === 'extract') {
        const observed = projectSnapshot(session, await snapshot(session, action.max_elements ?? 200));
        session.lastExtraction = observed;
        results.push({ index, type: action.type, status: 'completed' });
        continue;
      } else if (action.type === 'wait') {
        const duration = Math.min(30_000, Math.max(0, action.condition?.duration_ms ?? 500));
        await driver(session.runId, { op: 'sleep', milliseconds: duration }, duration + 2_000);
      } else if (['back', 'forward', 'reload'].includes(action.type)) {
        await driver(session.runId, { op: action.type });
        changed = true;
      } else {
        throw Object.assign(new Error(`unsupported Takoda browser action: ${action.type}`), { status: 400, code: 'invalid_request' });
      }
      results.push({ index, type: action.type, status: 'completed' });
    }

    if (changed) session.revision += 1;
    latest = await snapshot(session);
    const observed = projectSnapshot(session, latest);
    const response = {
      request_id: req.request_id,
      session_id: session.id,
      revision: session.revision,
      status: 'completed',
      page: observed.page,
      action_results: results,
      ...(session.lastScreenshot ? { screenshot: session.lastScreenshot } : {}),
      ...(session.lastExtraction ? { extracted: session.lastExtraction } : {}),
      changed,
      summary: `${results.filter((item) => item.status === 'completed').length} action(s) completed.`,
    };
    if (req.request_id) {
      session.idempotency.set(req.request_id, response);
      if (session.idempotency.size > 100) session.idempotency.delete(session.idempotency.keys().next().value);
    }
    return response;
  } catch (error) {
    if (!error.code) error.code = 'browser_action_failed';
    throw error;
  }
}

async function handleObserve(req) {
  const owner = String(req.owner ?? 'anonymous').slice(0, 200);
  const session = sessions.get(req.session_id);
  if (!session || session.owner !== owner) throw Object.assign(new Error('no such session'), { status: 404, code: 'session_not_found' });

  const since = req.since_revision;
  if (since !== undefined && since === session.revision && req.wait_ms) {
    const deadline = Date.now() + Math.min(30_000, Math.max(0, req.wait_ms));
    while (Date.now() < deadline && since === session.revision) {
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    if (since === session.revision) {
      const snap = await snapshot(session, req.max_elements ?? 200);
      return projectSnapshot(session, snap, since, true);
    }
  }
  const snap = await snapshot(session, req.max_elements ?? 200);
  return projectSnapshot(session, snap, since, false);
}

const server = createServer(async (req, res) => {
  try {
    const path = new URL(req.url ?? '/', 'http://localhost').pathname;
    if (req.method === 'GET' && path === '/agent/healthz') {
      try {
        const daemonStatus = await daemon('/v1/status', { timeoutMs: 3_000 });
        return sendJson(res, 200, {
          ok: true,
          service: 'tkda-browser-mcp-adapter',
          daemon: daemonStatus.ok === true ? 'ok' : 'degraded',
          sessions: sessions.size,
          maxSessions: MAX_SESSIONS,
          engines: ['playwright'],
        });
      } catch {
        return sendJson(res, 503, { ok: false, service: 'tkda-browser-mcp-adapter', daemon: 'unavailable' });
      }
    }

    if (!path.startsWith('/agent/')) return sendJson(res, 404, { error: 'not found' });
    if (!authorized(req)) return sendJson(res, 401, { error: 'unauthorized' });

    if (req.method === 'GET' && path === '/agent/tools') {
      return sendJson(res, 200, {
        tools: ['browser_act', 'browser_state'],
        engines: ['playwright'],
        allowlistEnforced: true,
        transport: 'tkda-desktop-daemon',
      });
    }

    if (req.method !== 'POST') return sendJson(res, 405, { error: 'method not allowed' });
    const body = await readJson(req);
    if (path === '/agent/act') return sendJson(res, 200, await handleAct(body));
    if (path === '/agent/observe') return sendJson(res, 200, await handleObserve(body));
    if (path === '/agent/close') {
      const session = sessions.get(body.session_id);
      if (!session || session.owner !== String(body.owner ?? 'anonymous').slice(0, 200)) {
        return sendJson(res, 404, { error_code: 'session_not_found', error: 'no such session' });
      }
      await daemon(`/v1/runs/${encodeURIComponent(session.runId)}/cancel`, { method: 'POST', body: {} });
      sessions.delete(session.id);
      return sendJson(res, 200, { ok: true, session_id: session.id });
    }
    return sendJson(res, 404, { error: 'not found' });
  } catch (error) {
    const status = Number(error.status) || 500;
    const safeStatus = status >= 400 && status <= 599 ? status : 500;
    sendJson(res, safeStatus, {
      error_code: error.code ?? (safeStatus === 500 ? 'internal_error' : 'request_failed'),
      error: safeStatus === 500 ? 'internal error' : String(error.message ?? 'request failed').slice(0, 500),
    });
  }
});

server.listen(bind.port, bind.host, () => {
  process.stderr.write(`tkda-browser-mcp-adapter listening on http://${bind.host}:${bind.port}\n`);
});
