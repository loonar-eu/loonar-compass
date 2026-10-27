// Loonar Compass feedback service (Cloudflare Worker).
//
//   POST /v1/start   the start of the application: installation, version, system, language -> the D1 database
//   POST /v1/stats   the usage summary of a day of one installation (sent every hour, "partial" until the day ends)
//                    -> the D1 database (the newest summary of a day replaces the earlier one); without the database
//                       (tests, before the database exists) a JSON file in the public statistics repository
//   POST /v1/report  a bug report or a feature request             -> a public GitHub issue; the screenshots and the
//                                                                     context of the reported answer go to a private
//                                                                     repository that only the author can read
//   GET  /health
//
// The application never holds a GitHub token: the token is a secret of this Worker (GITHUB_TOKEN). Everything that
// arrives is checked against a strict schema; unknown fields are refused, so nothing else can be published.
// Addresses (IP) are used only by the rate limiter and are never stored.

const LANGS = ["en", "pl", "de", "fr"];
const SETUP_ENUMS = {
  mode: ["ea_tab", "standalone"],
  model_source: ["file", "postgres", "sqlserver", "mysql", "oracle", "cloud"],
  ai_provider: ["anthropic", "openai", "azure", "local", "openai_compatible"],
  approval: ["plan", "ask", "auto"],
};
const COUNTER_GROUPS = ["tools", "tool_errors", "views", "events", "approvals", "tokens", "tasks", "task_steps", "task_tokens", "task_seconds", "gaps", "ratings"];
const COUNTERS = ["turns", "ai_errors"];
const MAX_COUNT = 10_000_000;
const MAX_STATS_BYTES = 32 * 1024;
const MAX_REPORT_BYTES = 12 * 1024 * 1024;
const MAX_IMAGES = 5;
const MAX_IMAGE_BYTES = 3 * 1024 * 1024;

export class Invalid extends Error {}

const json = (status, body) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json; charset=utf-8" } });

function check(ok, message) {
  if (!ok) throw new Invalid(message);
}

const isObject = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
const isCount = (v) => Number.isInteger(v) && v >= 0 && v <= MAX_COUNT;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const VERSION = /^\d{1,3}(\.\d{1,3}){0,3}$/;
const KEY = /^[a-z0-9_]{1,40}$/;
const OS = /^[A-Za-z0-9 ().]{1,40}$/;
const MODEL = /^[a-z0-9._-]{1,40}$/;

function daysBetween(a, b) {
  return Math.round((Date.parse(a + "T00:00:00Z") - Date.parse(b + "T00:00:00Z")) / 86_400_000);
}

/** The daily summary, re-built from known fields only (anything unknown is refused). */
export function validateStats(input, now = new Date()) {
  check(isObject(input), "not an object");
  const allowed = new Set(["v", "install", "day", "app", "os", "lang", "setup", "partial", ...COUNTERS, ...COUNTER_GROUPS]);
  for (const k of Object.keys(input)) check(allowed.has(k), `unknown field ${k}`);
  check(input.v === 1, "unsupported version");
  check(typeof input.install === "string" && UUID.test(input.install), "bad install");
  check(typeof input.day === "string" && /^\d{4}-\d{2}-\d{2}$/.test(input.day) && !isNaN(Date.parse(input.day)), "bad day");
  const today = now.toISOString().slice(0, 10);
  const age = daysBetween(today, input.day);
  check(age >= -1 && age <= 35, "day out of range");
  check(typeof input.app === "string" && VERSION.test(input.app), "bad app");
  check(typeof input.os === "string" && OS.test(input.os), "bad os");
  check(LANGS.includes(input.lang), "bad lang");
  const out = { v: 1, install: input.install, day: input.day, app: input.app, os: input.os, lang: input.lang };
  if (input.partial !== undefined) {
    check(typeof input.partial === "boolean", "bad partial");
    out.partial = input.partial;
  }
  if (input.setup !== undefined) {
    const s = input.setup;
    check(isObject(s), "bad setup");
    const keys = ["mode", "model_source", "ai_provider", "ai_model", "approval", "jira", "confluence", "hub", "mcp_servers", "event_agents"];
    for (const k of Object.keys(s)) check(keys.includes(k), `unknown setup field ${k}`);
    const setup = {};
    for (const [k, values] of Object.entries(SETUP_ENUMS)) {
      if (s[k] !== undefined) {
        check(values.includes(s[k]), `bad setup.${k}`);
        setup[k] = s[k];
      }
    }
    if (s.ai_model !== undefined) {
      check(typeof s.ai_model === "string" && MODEL.test(s.ai_model), "bad setup.ai_model");
      setup.ai_model = s.ai_model;
    }
    for (const k of ["jira", "confluence", "hub"]) {
      if (s[k] !== undefined) {
        check(typeof s[k] === "boolean", `bad setup.${k}`);
        setup[k] = s[k];
      }
    }
    for (const k of ["mcp_servers", "event_agents"]) {
      if (s[k] !== undefined) {
        check(Number.isInteger(s[k]) && s[k] >= 0 && s[k] <= 100, `bad setup.${k}`);
        setup[k] = s[k];
      }
    }
    out.setup = setup;
  }
  for (const k of COUNTERS) {
    if (input[k] !== undefined) {
      check(isCount(input[k]), `bad ${k}`);
      out[k] = input[k];
    }
  }
  for (const group of COUNTER_GROUPS) {
    if (input[group] === undefined) continue;
    const g = input[group];
    check(isObject(g) && Object.keys(g).length <= 120, `bad ${group}`);
    out[group] = {};
    for (const [k, v] of Object.entries(g)) {
      check(KEY.test(k), `bad key in ${group}`);
      check(isCount(v), `bad count in ${group}`);
      out[group][k] = v;
    }
  }
  return out;
}

/** The start of the application: the installation, its version, system and language, and the day. */
export function validateStart(input, now = new Date()) {
  check(isObject(input), "not an object");
  const allowed = new Set(["v", "install", "app", "os", "lang", "day"]);
  for (const k of Object.keys(input)) check(allowed.has(k), `unknown field ${k}`);
  check(input.v === 1, "unsupported version");
  check(typeof input.install === "string" && UUID.test(input.install), "bad install");
  check(typeof input.app === "string" && VERSION.test(input.app), "bad app");
  check(typeof input.os === "string" && OS.test(input.os), "bad os");
  check(LANGS.includes(input.lang), "bad lang");
  check(typeof input.day === "string" && /^\d{4}-\d{2}-\d{2}$/.test(input.day), "bad day");
  const age = daysBetween(now.toISOString().slice(0, 10), input.day);
  check(age >= -1 && age <= 2, "day out of range");
  return { install: input.install, app: input.app, os: input.os, lang: input.lang, day: input.day };
}

/** A user mention (@name) in a public issue would notify that GitHub user: mentions are broken with a zero-width space. */
export function noMentions(text) {
  return text.replace(/@(?=[A-Za-z0-9-])/g, "@​");
}

function decodeImage(image) {
  check(isObject(image) && typeof image.data === "string", "bad image");
  const bytes = Uint8Array.from(atob(image.data), (c) => c.charCodeAt(0));
  check(bytes.length > 0 && bytes.length <= MAX_IMAGE_BYTES, "image too large");
  const png = bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47;
  const jpg = bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
  check(png || jpg, "only PNG and JPEG images");
  return { bytes, ext: png ? "png" : "jpg" };
}

/** A bug report, checked: public parts (title, description, technical) and private parts (context, images). */
export function validateReport(input) {
  check(isObject(input), "not an object");
  const allowed = new Set(["v", "install", "app", "os", "lang", "kind", "title", "description", "technical", "private", "images"]);
  for (const k of Object.keys(input)) check(allowed.has(k), `unknown field ${k}`);
  check(input.v === 1, "unsupported version");
  check(typeof input.install === "string" && UUID.test(input.install), "bad install");
  check(typeof input.app === "string" && VERSION.test(input.app), "bad app");
  check(typeof input.os === "string" && OS.test(input.os), "bad os");
  check(LANGS.includes(input.lang), "bad lang");
  const text = (v, max, name, required) => {
    if (v === undefined || v === null) {
      check(!required, `missing ${name}`);
      return "";
    }
    check(typeof v === "string" && v.length <= max, `bad ${name}`);
    check(!required || v.trim().length > 0, `missing ${name}`);
    return v;
  };
  check(input.kind === undefined || ["bug", "feature"].includes(input.kind), "bad kind");
  const report = {
    kind: input.kind || "bug",
    install: input.install,
    app: input.app,
    os: input.os,
    lang: input.lang,
    title: text(input.title, 120, "title", true).replace(/[\r\n]+/g, " ").trim(),
    description: text(input.description, 10_000, "description", true),
    technical: text(input.technical, 2_000, "technical", false),
    private: text(input.private, 40_000, "private", false),
    images: [],
  };
  if (input.images !== undefined) {
    check(Array.isArray(input.images) && input.images.length <= MAX_IMAGES, "too many images");
    report.images = input.images.map(decodeImage);
  }
  return report;
}

async function github(env, method, path, body) {
  const api = env.GITHUB_API || "https://api.github.com";
  const response = await fetch(api + path, {
    method,
    headers: {
      Authorization: `Bearer ${env.GITHUB_TOKEN}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "loonar-compass-feedback",
      "Content-Type": "application/json",
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await response.json().catch(() => ({}));
  return { status: response.status, data };
}

function base64(bytes) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** Creates or replaces a file in a repository (a day sent again replaces the earlier summary of that day). */
async function putFile(env, repo, path, bytes, message) {
  const url = `/repos/${repo}/contents/${path}`;
  const body = { message, content: base64(bytes) };
  let r = await github(env, "PUT", url, body);
  if (r.status === 422 || r.status === 409) {
    const existing = await github(env, "GET", url);
    if (existing.status === 200 && existing.data.sha) r = await github(env, "PUT", url, { ...body, sha: existing.data.sha });
  }
  if (r.status !== 200 && r.status !== 201) throw new Error(`GitHub ${r.status} for ${path}`);
}

async function readBody(request, max) {
  const length = Number(request.headers.get("Content-Length") || "0");
  if (length > max) throw new Invalid("too large");
  const text = await request.text();
  if (text.length > max) throw new Invalid("too large");
  try {
    return JSON.parse(text);
  } catch {
    throw new Invalid("not JSON");
  }
}

async function limited(limiter, key) {
  if (!limiter) return false;
  const { success } = await limiter.limit({ key });
  return !success;
}

async function stats(request, env) {
  const ip = request.headers.get("CF-Connecting-IP") || "local";
  if (await limited(env.STATS_LIMITER, ip)) return json(429, { error: "too many requests" });
  const summary = validateStats(await readBody(request, MAX_STATS_BYTES));
  if (env.DB) {
    // The newest summary of a day of an installation; a final one is not replaced by a late partial one.
    await env.DB.prepare(
      `INSERT INTO days (install, day, app, partial, data, updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
       ON CONFLICT (install, day) DO UPDATE SET app = excluded.app, partial = excluded.partial, data = excluded.data, updated = excluded.updated
       WHERE days.partial = 1 OR excluded.partial = 0`,
    )
      .bind(summary.install, summary.day, summary.app, summary.partial ? 1 : 0, JSON.stringify(summary), new Date().toISOString())
      .run();
    return json(201, { ok: true });
  }
  const [y, m] = summary.day.split("-");
  const path = `data/${y}/${m}/${summary.day}/${summary.install}.json`;
  const bytes = new TextEncoder().encode(JSON.stringify(summary, null, 2) + "\n");
  await putFile(env, env.STATS_REPO, path, bytes, `stats ${summary.day} ${summary.install.slice(0, 8)}`);
  return json(201, { ok: true });
}

async function start(request, env) {
  const ip = request.headers.get("CF-Connecting-IP") || "local";
  if (await limited(env.STATS_LIMITER, ip)) return json(429, { error: "too many requests" });
  const s = validateStart(await readBody(request, 2048));
  if (!env.DB) return json(503, { error: "no database" });
  await env.DB.prepare("INSERT INTO starts (install, day, app, os, lang, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
    .bind(s.install, s.day, s.app, s.os, s.lang, new Date().toISOString())
    .run();
  return json(201, { ok: true });
}

async function report(request, env) {
  const ip = request.headers.get("CF-Connecting-IP") || "local";
  if (await limited(env.REPORT_LIMITER, ip)) return json(429, { error: "too many reports from this address, try again in a minute" });
  const r = validateReport(await readBody(request, MAX_REPORT_BYTES));
  const ref = crypto.randomUUID().slice(0, 8);
  const hasPrivate = r.private.trim().length > 0 || r.images.length > 0;
  const lines = [noMentions(r.description.trim()), "", "---"];
  if (r.technical.trim()) lines.push("```", r.technical.trim().replace(/```/g, "'''"), "```");
  lines.push(`Version ${r.app} · ${r.os} · language ${r.lang} · installation \`${r.install}\``);
  if (hasPrivate) {
    const parts = [];
    if (r.images.length) parts.push(`${r.images.length} screenshot(s)`);
    if (r.private.trim()) parts.push("the reported answer");
    lines.push(`Visible to the maintainer only: ${parts.join(" and ")} (ref \`${ref}\`).`);
  }
  lines.push("", "_Sent from Loonar Compass._");
  const issue = await github(env, "POST", `/repos/${env.ISSUES_REPO}/issues`, {
    title: `[${r.app}] ${r.title}`,
    body: lines.join("\n"),
    labels: [r.kind === "feature" ? "idea" : "bug", "from-app"],
  });
  if (issue.status !== 201) throw new Error(`GitHub ${issue.status} creating the issue`);
  const number = issue.data.number;
  if (hasPrivate) {
    const folder = `reports/${new Date().toISOString().slice(0, 7)}/issue-${number}-${ref}`;
    const text = [
      `# Issue #${number}: ${r.title}`,
      "",
      issue.data.html_url,
      "",
      `Installation: ${r.install} · version ${r.app} · ${r.os} · ${r.lang}`,
      "",
      "## Private part",
      "",
      r.private.trim() || "(none)",
      "",
      ...r.images.map((img, i) => `![screenshot ${i + 1}](${i + 1}.${img.ext})`),
      "",
    ].join("\n");
    try {
      await putFile(env, env.PRIVATE_REPO, `${folder}/report.md`, new TextEncoder().encode(text), `report ${ref} for #${number}`);
      for (const [i, img] of r.images.entries()) {
        await putFile(env, env.PRIVATE_REPO, `${folder}/${i + 1}.${img.ext}`, img.bytes, `screenshot ${i + 1} ${ref}`);
      }
    } catch (e) {
      // The issue exists: the reporter gets its address, the maintainer sees the problem in the Worker's logs.
      console.error(`private part of #${number} (${ref}) not stored: ${e}`);
    }
  }
  return json(201, { url: issue.data.html_url, number });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    try {
      if (request.method === "GET" && url.pathname === "/health") return json(200, { ok: true });
      if (request.method === "POST" && url.pathname === "/v1/start") return await start(request, env);
      if (request.method === "POST" && url.pathname === "/v1/stats") return await stats(request, env);
      if (request.method === "POST" && url.pathname === "/v1/report") return await report(request, env);
      return json(404, { error: "not found" });
    } catch (e) {
      if (e instanceof Invalid) return json(400, { error: e.message });
      console.error(e);
      return json(502, { error: "the service could not store it, try again later" });
    }
  },
};
