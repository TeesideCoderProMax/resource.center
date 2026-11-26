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
const MODEL_SLUG = "[a-z0-9][a-z0-9-]{0,62}[a-z0-9]";
const UNIT_ID = "[A-Za-z0-9._-]+";
const HOUR = 3600_000;

const MODELS = [
  { id: "dell-latitude-7440", name: "Dell Latitude 7440", category: "laptop", description: "14-inch business laptop", loanDurationDays: 14, units: 2, imageUrl: "/catalogue/dell-latitude-7440.webp", imageAlt: "Dell Latitude 7440", specifications: [{ label: "Configuration", value: "14-inch business laptop" }] },
  { id: "ipad-air-m2", name: "iPad Air M2", category: "tablet", description: "11-inch tablet with pencil", loanDurationDays: 14, units: 1, imageUrl: "/catalogue/ipad-air-m2.webp", imageAlt: "iPad Air M2", specifications: [{ label: "Configuration", value: "11-inch tablet with pencil" }] },
  { id: "canon-r50", name: "Canon EOS R50", category: "camera", description: "Mirrorless camera kit", loanDurationDays: 14, units: 0, imageUrl: "/catalogue/canon-eos-r50.webp", imageAlt: "Canon EOS R50", specifications: [{ label: "Configuration", value: "Mirrorless camera kit" }] },
];

let state;
export function reset() {
  state = { models: structuredClone(MODELS), units: [], reservations: [], subscriptions: [], events: [] };
  for (const m of state.models) {
    for (let i = 1; i <= Math.max(m.units, 1); i++) {
      state.units.push({
        id: `${m.id}-u${i}`,
        modelId: m.id,
        serialNumber: `FAKE-${m.id.toUpperCase()}-${i}`,
        status: m.units === 0 ? "MAINTENANCE" : "AVAILABLE",
      });
    }
  }
}
reset();

const summary = (m) => ({
  id: m.id, name: m.name, category: m.category, description: m.description, loanDurationDays: m.loanDurationDays,
  totalUnits: state.units.filter((u) => u.modelId === m.id).length,
  imageUrl: m.imageUrl,
  imageAlt: m.imageAlt,
  specifications: m.specifications,
  availableUnits: state.units.filter((u) => u.modelId === m.id && u.status === "AVAILABLE").length,
});

const staffInventorySummary = (m) => ({
  ...summary(m),
  units: state.units
    .filter((u) => u.modelId === m.id)
    .map((u) => ({
      unitId: u.id,
      serialNumber: u.serialNumber,
      status: u.status === "LOANED" ? "COLLECTED" : u.status,
    })),
});

function validateImageUrl(raw) {
  const s = typeof raw === "string" ? raw.trim() : "";
  if (!s || s.length > 700_000) return false;
  if (s.startsWith("/catalogue/")) return s.length > "/catalogue/".length && !s.includes("..") && !s.includes("\\") && !/\s/.test(s);
  if (s.startsWith("https://")) return s.length > "https://".length && s.length <= 2048 && !/\s/.test(s);
  return /^data:image\/(webp|png|jpeg);base64,[A-Za-z0-9+/=]+$/.test(s);
}

function notifySubscribersIfRestored(model, wasAvailableCount) {
  if (wasAvailableCount !== 0) return;
  const subs = state.subscriptions.filter((s) => s.modelId === model.id && s.status === "ACTIVE");
  for (const email of new Set(subs.map((s) => s.email))) {
    state.events.push({ eventId: `evt-${randomUUID()}`, type: "DeviceAvailabilityNotificationRequested", recipient: email, deviceModel: model.name });
  }
  subs.forEach((s) => (s.status = "NOTIFIED"));
}

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
    if (!Number.isInteger(body.loanDurationDays) || body.loanDurationDays < 1 || body.loanDurationDays > model.loanDurationDays) return [400, { error: "InvalidInput", message: "Choose 1–14 whole days" }];
    if (!unit) return [409, { error: "NoAvailableUnits", message: "no units available" }];
    unit.status = "RESERVED";
    const now = new Date();
    const r = { reservationId: randomUUID(), modelId: model.id, unitId: unit.id, studentSub: a.sub, studentEmail: a.email, status: "RESERVED",
      reservedAt: now.toISOString(), dueAt: new Date(now.getTime() + body.loanDurationDays * 24 * HOUR).toISOString(), collectedAt: null, returnedAt: null };
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

  if (path.startsWith("/api/v1/staff/inventory")) {
    if (!a.roles.includes("staff")) return [403, { error: "ForbiddenRole", message: "staff role required" }];
    if (req.method === "GET" && path === "/api/v1/staff/inventory") {
      return [200, { items: state.models.map(staffInventorySummary), correlationId: cid }];
    }
    if (req.method === "POST" && path === "/api/v1/staff/inventory/models") {
      const id = typeof body.id === "string" ? body.id.trim() : "";
      const name = typeof body.name === "string" ? body.name.trim() : "";
      const category = typeof body.category === "string" ? body.category.trim() : "";
      const description = typeof body.description === "string" ? body.description.trim() : "";
      const imageAlt = typeof body.imageAlt === "string" ? body.imageAlt.trim() : "";
      if (!new RegExp(`^${MODEL_SLUG}$`).test(id) || id.includes("--")) return [400, { error: "InvalidInput", message: "id must be 2–64 lowercase letters, digits, or single hyphens" }];
      if (!name || !category || !description || !imageAlt) return [400, { error: "InvalidInput", message: "name, category, description, and imageAlt are required" }];
      if (!Number.isInteger(body.loanDurationDays) || body.loanDurationDays < 1 || body.loanDurationDays > 14) return [400, { error: "InvalidInput", message: "loanDurationDays must be between 1 and 14" }];
      if (!validateImageUrl(body.imageUrl)) return [400, { error: "InvalidInput", message: "imageUrl must be a /catalogue/ path, https:// URL, or base64 webp/png/jpeg image" }];
      if (!Array.isArray(body.specifications) || body.specifications.length === 0 || body.specifications.some((s) => !s?.label?.trim() || !s?.value?.trim())) {
        return [400, { error: "InvalidInput", message: "at least 1 complete specification is required" }];
      }
      if (!Array.isArray(body.units) || body.units.length === 0) return [400, { error: "InvalidInput", message: "at least 1 initial unit is required" }];
      const normalizedUnits = [];
      const seen = new Set();
      for (const u of body.units) {
        const serial = typeof u?.serialNumber === "string" ? u.serialNumber.trim() : "";
        const status = typeof u?.status === "string" ? u.status.trim().toUpperCase() : "";
        if (!serial || (status !== "AVAILABLE" && status !== "MAINTENANCE")) return [400, { error: "InvalidInput", message: "each unit requires serialNumber and status AVAILABLE or MAINTENANCE" }];
        if (seen.has(serial.toLowerCase())) return [400, { error: "InvalidInput", message: "unit serial numbers must be unique" }];
        seen.add(serial.toLowerCase());
        normalizedUnits.push({ serialNumber: serial, status });
      }
      if (state.models.some((x) => x.id === id)) return [409, { error: "AlreadyExists", message: `device model '${id}' already exists` }];
      if (normalizedUnits.some((u) => state.units.some((existing) => existing.serialNumber.toLowerCase() === u.serialNumber.toLowerCase()))) {
        return [409, { error: "AlreadyExists", message: "a unit with this serial number already exists" }];
      }
      const created = {
        id,
        name,
        category,
        description,
        loanDurationDays: body.loanDurationDays,
        imageUrl: body.imageUrl.trim(),
        imageAlt,
        specifications: body.specifications.map((s) => ({ label: s.label.trim(), value: s.value.trim() })),
      };
      state.models.push(created);
      normalizedUnits.forEach((u, idx) => {
        state.units.push({ id: `campus-${id}-${String(idx + 1).padStart(3, "0")}`, modelId: id, serialNumber: u.serialNumber, status: u.status });
      });
      return [201, staffInventorySummary(created)];
    }
    const delModel = path.match(new RegExp(`^/api/v1/staff/inventory/models/(${MODEL_SLUG})$`));
    if (req.method === "DELETE" && delModel) {
      const modelId = delModel[1];
      if (!state.models.some((x) => x.id === modelId)) return [404, { error: "NotFound", message: "model not found" }];
      if (state.units.some((u) => u.modelId === modelId && (u.status === "RESERVED" || u.status === "LOANED"))) {
        return [409, { error: "ResourceInUse", message: "cannot remove a model while units are reserved or collected" }];
      }
      if (state.reservations.some((r) => r.modelId === modelId)) {
        return [409, { error: "ResourceInUse", message: "cannot remove a model with past loan records; move its units to maintenance instead" }];
      }
      state.subscriptions = state.subscriptions.filter((s) => s.modelId !== modelId);
      state.units = state.units.filter((u) => u.modelId !== modelId);
      state.models = state.models.filter((x) => x.id !== modelId);
      return [200, { deleted: true, correlationId: cid }];
    }
    const addUnit = path.match(new RegExp(`^/api/v1/staff/inventory/models/(${MODEL_SLUG})/units$`));
    if (req.method === "POST" && addUnit) {
      const model = state.models.find((x) => x.id === addUnit[1]);
      if (!model) return [404, { error: "NotFound", message: "model not found" }];
      const serial = typeof body.serialNumber === "string" ? body.serialNumber.trim() : "";
      const status = typeof body.status === "string" ? body.status.trim().toUpperCase() : "";
      if (!serial || (status !== "AVAILABLE" && status !== "MAINTENANCE")) return [400, { error: "InvalidInput", message: "serialNumber and status AVAILABLE or MAINTENANCE are required" }];
      if (state.units.some((u) => u.serialNumber.toLowerCase() === serial.toLowerCase())) return [409, { error: "AlreadyExists", message: "a unit with this serial number already exists" }];
      const wasAvail = state.units.filter((u) => u.modelId === model.id && u.status === "AVAILABLE").length;
      const unit = { id: `unit-${model.id}-${randomUUID().slice(0, 8)}`, modelId: model.id, serialNumber: serial, status };
      state.units.push(unit);
      if (status === "AVAILABLE") notifySubscribersIfRestored(model, wasAvail);
      return [201, { unitId: unit.id, serialNumber: unit.serialNumber, status: unit.status }];
    }
    const unitRoute = path.match(new RegExp(`^/api/v1/staff/inventory/units/(${UNIT_ID})$`));
    if (unitRoute) {
      const unit = state.units.find((u) => u.id === unitRoute[1]);
      if (!unit) return [404, { error: "NotFound", message: "unit not found" }];
      const model = state.models.find((x) => x.id === unit.modelId);
      if (req.method === "PATCH") {
        const target = typeof body.status === "string" ? body.status.trim().toUpperCase() : "";
        if (target !== "AVAILABLE" && target !== "MAINTENANCE") return [400, { error: "InvalidInput", message: "status must be AVAILABLE or MAINTENANCE" }];
        if (unit.status === "RESERVED" || unit.status === "LOANED") return [409, { error: "InvalidTransition", message: "cannot change status of a reserved or collected unit" }];
        const wasAvail = state.units.filter((u) => u.modelId === unit.modelId && u.status === "AVAILABLE").length;
        const prev = unit.status;
        unit.status = target;
        if (prev === "MAINTENANCE" && target === "AVAILABLE") notifySubscribersIfRestored(model, wasAvail);
        return [200, { unitId: unit.id, serialNumber: unit.serialNumber, status: unit.status }];
      }
      if (req.method === "DELETE") {
        if (unit.status === "RESERVED" || unit.status === "LOANED") return [409, { error: "ResourceInUse", message: "cannot remove a unit that is currently reserved or out on loan" }];
        if (state.reservations.some((r) => r.unitId === unit.id)) return [409, { error: "ResourceInUse", message: "cannot remove a unit with past loan records; move it to maintenance instead" }];
        state.units = state.units.filter((u) => u.id !== unit.id);
        return [200, { deleted: true, correlationId: cid }];
      }
    }
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
