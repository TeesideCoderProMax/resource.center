/**
 * Fake Loan API for component/E2E tests and `npm run dev:fake`. Implements the contract
 * routes and error envelopes in docs/backend/contracts.md with in-memory state.
 * It does NOT verify JWT signatures: it decodes the bearer payload for sub/email/roles only.
 * Test-only; never deploy it. Control routes: POST /__reset, POST /__seed.
 */
import { createServer } from "node:http";
import { randomUUID } from "node:crypto";

const ROLES_CLAIM = process.env.AUTH0_ROLES_CLAIM || "https://resource-center.campus/roles";
const EMAIL_CLAIM = process.env.AUTH0_EMAIL_CLAIM || "https://resource-center.campus/email";
const UUID = "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";
const HOUR = 3600_000;

const MODELS = [
  { id: "dell-latitude-7440", name: "Dell Latitude 7440", category: "laptop", description: "14-inch business laptop", loanDurationDays: 2, units: 2 },
  { id: "ipad-air-m2", name: "iPad Air M2", category: "tablet", description: "11-inch tablet with pencil", loanDurationDays: 2, units: 1 },
  { id: "canon-r50", name: "Canon EOS R50", category: "camera", description: "Mirrorless camera kit", loanDurationDays: 2, units: 0 },
];

let state;
export function reset() {
  state = { models: structuredClone(MODELS), units: [], reservations: [], subscriptions: [], events: [] };
  for (const m of state.models) {
    for (let i = 1; i <= Math.max(m.units, 1); i++) state.units.push({ id: `${m.id}-u${i}`, modelId: m.id, status: m.units === 0 ? "MAINTENANCE" : "AVAILABLE" });
  }
}
reset();

const summary = (m) => ({
  id: m.id, name: m.name, category: m.category, description: m.description, loanDurationDays: m.loanDurationDays,
  totalUnits: state.units.filter((u) => u.modelId === m.id && u.status !== "MAINTENANCE").length,
  availableUnits: state.units.filter((u) => u.modelId === m.id && u.status === "AVAILABLE").length,
});

function actor(req) {
  const h = req.headers.authorization ?? "";
  if (!h.startsWith("Bearer ")) return { error: [401, "Unauthorized", "missing bearer token"] };
  try {
    const p = JSON.parse(Buffer.from(h.slice(7).split(".")[1], "base64url").toString("utf8"));
    if (!p.sub || !p[EMAIL_CLAIM]) return { error: [401, "InvalidToken", "token is not valid"] };
    const roles = Array.isArray(p[ROLES_CLAIM]) ? p[ROLES_CLAIM].map((r) => String(r).toLowerCase()) : [];
    return { sub: p.sub, email: p[EMAIL_CLAIM], roles };
  } catch {
    return { error: [401, "InvalidToken", "token is not valid"] };
  }
}

const member = (a) => a.roles.includes("student") || a.roles.includes("staff");

async function handle(req, cid) {
  const url = new URL(req.url, "http://fake");
  const path = url.pathname;
  const raw = await new Promise((r) => { let b = ""; req.on("data", (c) => (b += c)); req.on("end", () => r(b)); });
  const body = raw ? JSON.parse(raw) : {};

  if (req.method === "POST" && path === "/__reset") { reset(); return [204]; }
  if (req.method === "POST" && path === "/__seed") {
    // Seed a reservation directly, e.g. an overdue loan: {modelId, sub, email, status, reservedHoursAgo}
    const unit = state.units.find((u) => u.modelId === body.modelId && u.status === "AVAILABLE");
    if (!unit) return [409, { error: "NoAvailableUnits", message: "no unit" }];
    unit.status = body.status === "COLLECTED" ? "LOANED" : "RESERVED";
    const reservedAt = new Date(Date.now() - (body.reservedHoursAgo ?? 0) * HOUR);
    const r = { reservationId: randomUUID(), modelId: body.modelId, unitId: unit.id, studentSub: body.sub, studentEmail: body.email,
      status: body.status ?? "RESERVED", reservedAt: reservedAt.toISOString(), dueAt: new Date(reservedAt.getTime() + 48 * HOUR).toISOString(),
      collectedAt: body.status === "COLLECTED" ? reservedAt.toISOString() : null, returnedAt: null };
    state.reservations.unshift(r);
    return [201, r];
  }
  if (req.method === "GET" && path === "/__events") return [200, state.events];
  if (req.method === "GET" && path === "/health/live") return [200, { status: "ok" }];
  if (req.method === "GET" && path === "/api/v1/catalogue") {
    return [200, { items: state.models.map(summary), correlationId: cid, auth0Configured: true }];
  }

  const a = actor(req);
  if (a.error) return [a.error[0], { error: a.error[1], message: a.error[2] }];
  if (!member(a)) return [403, { error: "ForbiddenRole", message: "role not allowed" }];

  if (req.method === "GET" && path === "/api/v1/reservations") {
    const rows = state.reservations.filter((r) => a.roles.includes("staff") || r.studentSub === a.sub);
    return [200, rows.map((r) => ({ ...r, correlationId: cid }))];
  }
  if (req.method === "POST" && (path === "/api/v1/reservations" || path === "/api/v1/subscriptions")) {
    if (typeof body.modelId !== "string" || !body.modelId.trim()) return [400, { error: "InvalidInput", message: "modelId is required" }];
    const model = state.models.find((m) => m.id === body.modelId);
    if (!model) return [404, { error: "NotFound", message: "model not found" }];
    const unit = state.units.find((u) => u.modelId === model.id && u.status === "AVAILABLE");
    if (path === "/api/v1/subscriptions") {
      if (unit) return [409, { error: "UnitsCurrentlyAvailable", message: "units are available" }];
      let s = state.subscriptions.find((x) => x.modelId === model.id && x.sub === a.sub && x.status === "ACTIVE");
      if (!s) state.subscriptions.push((s = { subscriptionId: randomUUID(), modelId: model.id, sub: a.sub, email: a.email, status: "ACTIVE", createdAt: new Date().toISOString() }));
      return [201, { subscriptionId: s.subscriptionId, modelId: s.modelId, status: s.status, createdAt: s.createdAt }];
    }
    if (!unit) return [409, { error: "NoAvailableUnits", message: "no units available" }];
    unit.status = "RESERVED";
    const now = new Date();
    const r = { reservationId: randomUUID(), modelId: model.id, unitId: unit.id, studentSub: a.sub, studentEmail: a.email, status: "RESERVED",
      reservedAt: now.toISOString(), dueAt: new Date(now.getTime() + 48 * HOUR).toISOString(), collectedAt: null, returnedAt: null };
    state.reservations.unshift(r);
    return [201, { reservationId: r.reservationId, modelId: r.modelId, unitId: r.unitId, status: r.status, reservedAt: r.reservedAt, dueAt: r.dueAt, correlationId: cid }];
  }
  const m = path.match(new RegExp(`^/api/v1/reservations/(${UUID})/(collect|return)$`));
  if (req.method === "POST" && m) {
    if (!a.roles.includes("staff")) return [403, { error: "ForbiddenRole", message: "staff role required" }];
    const r = state.reservations.find((x) => x.reservationId === m[1]);
    if (!r) return [404, { error: "NotFound", message: "reservation not found" }];
    const unit = state.units.find((u) => u.id === r.unitId);
    const model = state.models.find((x) => x.id === r.modelId);
    const now = new Date().toISOString();
    if (m[2] === "collect") {
      if (r.status !== "RESERVED") return [409, { error: "InvalidTransition", message: "not RESERVED" }];
      r.status = "COLLECTED"; r.collectedAt = now; unit.status = "LOANED";
      const ev = { eventId: `evt-${randomUUID()}`, type: "DeviceCollectionConfirmationRequested", recipient: r.studentEmail, deviceModel: model.name };
      state.events.push(ev);
      return [200, { reservationId: r.reservationId, unitId: r.unitId, status: r.status, collectedAt: now, dueAt: r.dueAt, outboxEventId: ev.eventId }];
    }
    if (r.status !== "COLLECTED") return [409, { error: "InvalidTransition", message: "not COLLECTED" }];
    r.status = "RETURNED"; r.returnedAt = now; unit.status = "AVAILABLE";
    const ids = [];
    const push = (type, recipient) => { const id = `evt-${randomUUID()}`; ids.push(id); state.events.push({ eventId: id, type, recipient, deviceModel: model.name }); };
    push("DeviceReturnConfirmationRequested", r.studentEmail);
    const subs = state.subscriptions.filter((s) => s.modelId === model.id && s.status === "ACTIVE");
    for (const email of new Set(subs.map((s) => s.email))) push("DeviceAvailabilityNotificationRequested", email);
    subs.forEach((s) => (s.status = "NOTIFIED"));
    return [200, { reservationId: r.reservationId, unitId: r.unitId, modelId: r.modelId, status: r.status, returnedAt: now, outboxEventIds: ids }];
  }
  return [404, { error: "NotFound", message: "unknown route" }];
}

export function start(port = Number(process.env.FAKE_LOAN_API_PORT || 4010)) {
  const server = createServer(async (req, res) => {
    const cid = req.headers["x-correlation-id"] || `fake-${randomUUID()}`;
    let status, payload;
    try {
      [status, payload] = await handle(req, cid);
    } catch {
      [status, payload] = [500, { error: "InternalError", message: "fake API error" }];
    }
    if (payload && status >= 400) payload.correlationId = cid;
    res.writeHead(status, { "content-type": "application/json", "x-correlation-id": cid });
    res.end(payload === undefined ? "" : JSON.stringify(payload));
  });
  return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve(server)));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  start().then((s) => console.log(`fake Loan API on http://127.0.0.1:${s.address().port}`));
}
