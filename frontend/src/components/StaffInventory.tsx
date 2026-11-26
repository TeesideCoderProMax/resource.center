"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { call } from "@/lib/api-client";
import type {
  CreateDeviceModelInput,
  DeviceUnitSummary,
  NewDeviceUnitInput,
  StaffInventoryModel,
} from "@/lib/loan";
import { categoryIcon, Icon, type IconName } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";
import { StaffSidebar } from "./StaffSidebar";

const DEFAULT_CATEGORIES = [
  "Laptop",
  "Tablet",
  "Camera",
  "Camera accessory",
  "Audio",
  "Presentation",
  "Accessory",
  "Lab equipment",
];

type StockFilter = "ALL" | "AVAILABLE" | "MAINTENANCE" | "WAITLIST";
type ImageMode = "upload" | "url";

interface SpecDraft {
  key: number;
  label: string;
  value: string;
}

interface UnitDraft {
  key: number;
  serialNumber: string;
  status: "AVAILABLE" | "MAINTENANCE";
  touched: boolean;
}

function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .replace(/-{2,}/g, "-")
    .slice(0, 64);
}

function defaultSerial(modelId: string, index: number): string {
  const prefix = (modelId || "ITEM").toUpperCase().replace(/[^A-Z0-9]+/g, "-").slice(0, 24);
  return `CAMPUS-${prefix}-${String(index + 1).padStart(3, "0")}`;
}

function validateImageUrlInput(raw: string): string | null {
  const s = raw.trim();
  if (!s) return "Choose or provide a product image.";
  if (s.length > 700_000) return "Product image must be at most 700 KB.";
  if (s.startsWith("/catalogue/")) {
    return s.length > "/catalogue/".length && !s.includes("..") && !s.includes("\\") && !/\s/.test(s)
      ? null
      : "Relative path must point to a valid /catalogue/ image.";
  }
  if (s.startsWith("https://")) {
    return s.length > "https://".length && s.length <= 2048 && !/\s/.test(s)
      ? null
      : "Image URL must be a valid https:// URL.";
  }
  if (/^data:image\/(webp|png|jpeg);base64,[A-Za-z0-9+/=]+$/.test(s)) return null;
  return "Image must be a WebP/PNG/JPEG upload, https:// URL, or /catalogue/ path.";
}

async function compressImageFile(file: File): Promise<string> {
  if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) {
    throw new Error("Please select a PNG, JPEG, or WebP image file.");
  }
  const rawUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(new Error("Could not read the selected image file."));
    reader.readAsDataURL(file);
  });

  // Resize/compress via canvas when running in a full browser; fall back to raw data URL if within limit.
  try {
    const img = await new Promise<HTMLImageElement>((resolve, reject) => {
      const el = new Image();
      el.onload = () => resolve(el);
      el.onerror = () => reject(new Error("decode-failed"));
      el.src = rawUrl;
    });
    const maxW = 960;
    const maxH = 640;
    const scale = Math.min(1, maxW / (img.naturalWidth || maxW), maxH / (img.naturalHeight || maxH));
    const width = Math.max(1, Math.round((img.naturalWidth || maxW) * scale));
    const height = Math.max(1, Math.round((img.naturalHeight || maxH) * scale));
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("no-2d-context");
    ctx.drawImage(img, 0, 0, width, height);
    let dataUrl = canvas.toDataURL("image/webp", 0.84);
    if (!dataUrl.startsWith("data:image/webp")) {
      dataUrl = canvas.toDataURL("image/jpeg", 0.84);
    }
    if (dataUrl.length <= 700_000) return dataUrl;
  } catch {
    // Fallback below when canvas decoding is unavailable (e.g., jsdom tests).
  }

  if (rawUrl.length <= 700_000) return rawUrl;
  throw new Error("Compressed image exceeds 700 KB; choose a smaller image.");
}

function unitToneClass(status: DeviceUnitSummary["status"]): string {
  switch (status) {
    case "AVAILABLE":
      return "pill-in";
    case "MAINTENANCE":
      return "pill-maint";
    default:
      return "pill-out";
  }
}

function StatCard({
  icon,
  label,
  value,
  unit,
  foot,
  tone,
}: {
  icon: IconName;
  label: string;
  value: number | string;
  unit: string;
  foot: React.ReactNode;
  tone?: "accent" | "danger";
}) {
  return (
    <div className={`glass stat-card${tone ? ` ${tone}` : ""}`}>
      <div className="row">
        <span className="muted small">{label}</span>
        <span className="stat-icon"><Icon name={icon} /></span>
      </div>
      <p className="stat-value"><strong>{value}</strong> <span className="muted">{unit}</span></p>
      <div className="small">{foot}</div>
    </div>
  );
}

export function StaffInventory({ initialModels }: { initialModels?: StaffInventoryModel[] }) {
  const router = useRouter();
  const [models, setModels] = useState<StaffInventoryModel[] | null>(initialModels ?? null);
  const [error, setError] = useState<NoticeValue>(null);
  const [notice, setNotice] = useState<NoticeValue>(null);

  const [query, setQuery] = useState("");
  const [categoryFilter, setCategoryFilter] = useState("");
  const [stockFilter, setStockFilter] = useState<StockFilter>("ALL");

  const [expandedModelId, setExpandedModelId] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<{ kind: "model" | "unit"; id: string } | null>(null);
  const [unitAddSerial, setUnitAddSerial] = useState<Record<string, string>>({});
  const [unitAddStatus, setUnitAddStatus] = useState<Record<string, "AVAILABLE" | "MAINTENANCE">>({});

  // Add equipment modal state (native <dialog>).
  const dialogRef = useRef<HTMLDialogElement>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const nextRowKey = useRef(1);
  const [creating, setCreating] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [modelId, setModelId] = useState("");
  const [idTouched, setIdTouched] = useState(false);
  const [category, setCategory] = useState("");
  const [loanDurationDays, setLoanDurationDays] = useState("14");
  const [description, setDescription] = useState("");
  const [imageMode, setImageMode] = useState<ImageMode>("upload");
  const [imageUrl, setImageUrl] = useState("");
  const [imageAlt, setImageAlt] = useState("");
  const [altTouched, setAltTouched] = useState(false);
  const [specs, setSpecs] = useState<SpecDraft[]>([{ key: 1, label: "", value: "" }]);
  const [units, setUnits] = useState<UnitDraft[]>([{ key: 2, serialNumber: "", status: "AVAILABLE", touched: false }]);

  const load = useCallback(async () => {
    try {
      const res = await call<{ items: StaffInventoryModel[] }>("GET", "staff/inventory");
      setModels(res.items);
      setError(null);
    } catch (e) {
      setError(errorNotice(e, "/staff/inventory"));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const allModels = models ?? [];
  const categories = useMemo(
    () => [...new Set(allModels.map((m) => m.category))].sort(),
    [allModels]
  );
  const categoryOptions = useMemo(
    () => [...new Set([...DEFAULT_CATEGORIES, ...categories])].sort(),
    [categories]
  );

  const summary = useMemo(() => {
    let totalUnits = 0;
    let availableUnits = 0;
    let maintenanceUnits = 0;
    let activeLoanUnits = 0;
    let waitlistModels = 0;
    for (const m of allModels) {
      totalUnits += m.totalUnits;
      availableUnits += m.availableUnits;
      if (m.availableUnits === 0) waitlistModels += 1;
      for (const u of m.units) {
        if (u.status === "MAINTENANCE") maintenanceUnits += 1;
        else if (u.status === "RESERVED" || u.status === "COLLECTED") activeLoanUnits += 1;
      }
    }
    return { totalUnits, availableUnits, maintenanceUnits, activeLoanUnits, waitlistModels };
  }, [allModels]);

  const filteredModels = useMemo(() => {
    const q = query.trim().toLowerCase();
    return allModels
      .filter((m) => !categoryFilter || m.category === categoryFilter)
      .filter((m) => {
        if (stockFilter === "AVAILABLE") return m.availableUnits > 0;
        if (stockFilter === "MAINTENANCE") return m.units.some((u) => u.status === "MAINTENANCE");
        if (stockFilter === "WAITLIST") return m.availableUnits === 0;
        return true;
      })
      .filter((m) => {
        if (!q) return true;
        const unitMatch = m.units.some(
          (u) => u.unitId.toLowerCase().includes(q) || u.serialNumber.toLowerCase().includes(q)
        );
        return unitMatch || `${m.id} ${m.name} ${m.description} ${m.category}`.toLowerCase().includes(q);
      });
  }, [allModels, query, categoryFilter, stockFilter]);

  function resetAddForm() {
    nextRowKey.current = 3;
    setFormError(null);
    setName("");
    setModelId("");
    setIdTouched(false);
    setCategory("");
    setLoanDurationDays("14");
    setDescription("");
    setImageMode("upload");
    setImageUrl("");
    setImageAlt("");
    setAltTouched(false);
    setSpecs([{ key: 1, label: "", value: "" }]);
    setUnits([{ key: 2, serialNumber: "", status: "AVAILABLE", touched: false }]);
  }

  function openAddModal() {
    resetAddForm();
    dialogRef.current?.showModal();
  }

  function updateName(nextName: string) {
    setName(nextName);
    const nextSlug = idTouched ? modelId : slugify(nextName);
    if (!idTouched) {
      setModelId(nextSlug);
      setUnits((cur) =>
        cur.map((u, idx) => (u.touched ? u : { ...u, serialNumber: nextSlug ? defaultSerial(nextSlug, idx) : "" }))
      );
    }
    if (!altTouched) {
      setImageAlt(nextName.trim() ? `${nextName.trim()} loan equipment` : "");
    }
  }

  function updateModelId(nextId: string) {
    setIdTouched(true);
    setModelId(nextId);
    const cleaned = slugify(nextId);
    setUnits((cur) =>
      cur.map((u, idx) => (u.touched ? u : { ...u, serialNumber: cleaned ? defaultSerial(cleaned, idx) : "" }))
    );
  }

  async function handleImageFile(file: File | undefined) {
    if (!file) {
      setImageUrl("");
      return;
    }
    try {
      setFormError(null);
      const dataUrl = await compressImageFile(file);
      setImageUrl(dataUrl);
    } catch (e) {
      setImageUrl("");
      setFormError(e instanceof Error ? e.message : "Could not process the image file.");
    }
  }

  async function handleCreateModel(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (creating) return;
    setFormError(null);

    const form = formRef.current;
    if (form && !form.checkValidity()) {
      const firstInvalid = form.querySelector<HTMLElement>(":invalid");
      firstInvalid?.focus();
      setFormError("Complete all mandatory fields before saving.");
      return;
    }

    const cleanId = modelId.trim();
    if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(cleanId) || cleanId.length < 2 || cleanId.length > 64) {
      form?.querySelector<HTMLElement>("#inv-add-id")?.focus();
      setFormError("Model ID must be 2–64 lowercase letters, digits, or single hyphens.");
      return;
    }

    const days = Number(loanDurationDays);
    if (!Number.isInteger(days) || days < 1 || days > 14) {
      form?.querySelector<HTMLElement>("#inv-add-days")?.focus();
      setFormError("Maximum loan period must be a whole number from 1 to 14 days.");
      return;
    }

    const imgErr = validateImageUrlInput(imageUrl);
    if (imgErr) {
      form?.querySelector<HTMLElement>("#inv-add-image")?.focus();
      setFormError(imgErr);
      return;
    }

    const cleanSpecs = specs.map((s) => ({ label: s.label.trim(), value: s.value.trim() }));
    if (cleanSpecs.length === 0 || cleanSpecs.some((s) => !s.label || !s.value)) {
      setFormError("Every specification row requires both a label and a value.");
      return;
    }

    const cleanUnits: NewDeviceUnitInput[] = units.map((u) => ({
      serialNumber: u.serialNumber.trim(),
      status: u.status,
    }));
    if (cleanUnits.length === 0 || cleanUnits.some((u) => !u.serialNumber)) {
      setFormError("Every initial stock unit requires a serial number.");
      return;
    }
    const serialSet = new Set(cleanUnits.map((u) => u.serialNumber.toLowerCase()));
    if (serialSet.size !== cleanUnits.length) {
      setFormError("Initial unit serial numbers must be unique.");
      return;
    }

    const payload: CreateDeviceModelInput = {
      id: cleanId,
      name: name.trim(),
      category: category.trim(),
      description: description.trim(),
      loanDurationDays: days,
      imageUrl: imageUrl.trim(),
      imageAlt: imageAlt.trim(),
      specifications: cleanSpecs,
      units: cleanUnits,
    };

    setCreating(true);
    try {
      const created = await call<StaffInventoryModel>("POST", "staff/inventory/models", payload);
      dialogRef.current?.close();
      setNotice({
        kind: "ok",
        text: `Added ${created.name} (${created.totalUnits} unit${created.totalUnits === 1 ? "" : "s"}).`,
      });
      setExpandedModelId(created.id);
      await load();
      router.refresh();
    } catch (err) {
      setFormError(errorNotice(err, "/staff/inventory")?.text ?? "Could not add equipment.");
    } finally {
      setCreating(false);
    }
  }

  async function toggleMaintenance(model: StaffInventoryModel, unit: DeviceUnitSummary) {
    if (busyKey) return;
    const target = unit.status === "MAINTENANCE" ? "AVAILABLE" : "MAINTENANCE";
    setBusyKey(unit.unitId);
    setNotice(null);
    try {
      await call<DeviceUnitSummary>("PATCH", `staff/inventory/units/${encodeURIComponent(unit.unitId)}`, {
        status: target,
      });
      setNotice({
        kind: "ok",
        text: target === "MAINTENANCE"
          ? `Moved ${unit.unitId} (${model.name}) to maintenance.`
          : `Returned ${unit.unitId} (${model.name}) to available stock.`,
      });
      await load();
      router.refresh();
    } catch (e) {
      setNotice(errorNotice(e, "/staff/inventory"));
    } finally {
      setBusyKey(null);
    }
  }

  async function handleAddUnit(e: React.FormEvent, model: StaffInventoryModel) {
    e.preventDefault();
    if (busyKey) return;
    const serial = (unitAddSerial[model.id] ?? "").trim();
    if (!serial) {
      setNotice({ kind: "error", text: "Enter a serial number before adding a unit." });
      return;
    }
    const status = unitAddStatus[model.id] ?? "AVAILABLE";
    setBusyKey(`add:${model.id}`);
    setNotice(null);
    try {
      const added = await call<DeviceUnitSummary>(
        "POST",
        `staff/inventory/models/${encodeURIComponent(model.id)}/units`,
        { serialNumber: serial, status }
      );
      setUnitAddSerial((prev) => ({ ...prev, [model.id]: "" }));
      setNotice({ kind: "ok", text: `Added unit ${added.unitId} (${added.serialNumber}) to ${model.name}.` });
      await load();
      router.refresh();
    } catch (err) {
      setNotice(errorNotice(err, "/staff/inventory"));
    } finally {
      setBusyKey(null);
    }
  }

  async function handleRemoveUnit(model: StaffInventoryModel, unit: DeviceUnitSummary) {
    if (busyKey) return;
    setBusyKey(unit.unitId);
    setNotice(null);
    try {
      await call("DELETE", `staff/inventory/units/${encodeURIComponent(unit.unitId)}`);
      setConfirmDelete(null);
      setNotice({ kind: "ok", text: `Removed unit ${unit.unitId} from ${model.name}.` });
      await load();
      router.refresh();
    } catch (e) {
      setConfirmDelete(null);
      setNotice(errorNotice(e, "/staff/inventory"));
    } finally {
      setBusyKey(null);
    }
  }

  async function handleRemoveModel(model: StaffInventoryModel) {
    if (busyKey) return;
    setBusyKey(`del-model:${model.id}`);
    setNotice(null);
    try {
      await call("DELETE", `staff/inventory/models/${encodeURIComponent(model.id)}`);
      setConfirmDelete(null);
      if (expandedModelId === model.id) setExpandedModelId(null);
      setNotice({ kind: "ok", text: `Removed ${model.name} (${model.id}) from the catalogue.` });
      await load();
      router.refresh();
    } catch (e) {
      setConfirmDelete(null);
      setNotice(errorNotice(e, "/staff/inventory"));
    } finally {
      setBusyKey(null);
    }
  }

  return (
    <div className="admin-layout">
      <StaffSidebar activePage="inventory" inventoryCount={models ? models.length : undefined} />

      <div className="admin-main">
        <section aria-labelledby="inv-page-h" className="page-head">
          <div>
            <nav aria-label="Breadcrumbs" className="crumbs">
              <ol>
                <li><Link href="/"><Icon name="home" size={14} /> Catalog</Link></li>
                <li><Link href="/staff/dashboard">Staff Admin</Link></li>
                <li><Link href="/staff/inventory" aria-current="page" className="accent">Inventory</Link></li>
              </ol>
            </nav>
            <h1 id="inv-page-h">Equipment inventory</h1>
            <p className="muted">Add new equipment models, manage physical units, and move devices in or out of maintenance.</p>
          </div>
          <div className="row-start">
            <button type="button" className="btn" onClick={() => load()}><Icon name="refresh" /> Refresh</button>
            <button type="button" className="btn primary" onClick={openAddModal}><Icon name="plus" /> Add equipment</button>
          </div>
        </section>

        <div className="stat-grid" role="region" aria-label="Inventory summary">
          <StatCard icon="box" label="Device models" value={allModels.length} unit="models"
            foot={<span className="muted">Across {categories.length} categories</span>} />
          <StatCard icon="laptop" label="Total stock" value={summary.totalUnits} unit="units"
            foot={<span className="good-text">{summary.availableUnits} available for loan</span>} />
          <StatCard icon="wrench" label="In maintenance" value={summary.maintenanceUnits} unit="units" tone="accent"
            foot={<span className="muted">{summary.activeLoanUnits} reserved or collected</span>} />
          <StatCard icon="alert" label="Waitlist only" value={summary.waitlistModels} unit="models" tone={summary.waitlistModels ? "danger" : undefined}
            foot={<span className="muted">Zero available units</span>} />
        </div>

        <Notice notice={notice} />
        <Notice notice={error} />

        <section aria-labelledby="inv-table-h" className="glass panel-lg">
          <div className="row row-wrap">
            <div>
              <h2 id="inv-table-h" className="h-sm"><Icon name="box" /> Catalogue &amp; unit fleet</h2>
              <p className="muted small">Expand any model to move units to maintenance, add new units, or remove unused stock.</p>
            </div>
          </div>

          <div className="filters">
            <div className="grow">
              <label htmlFor="inv-find" className="field-label">Filter models</label>
              <input
                id="inv-find"
                type="search"
                placeholder="Model ID, name, description or serial number"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </div>
            <div>
              <label htmlFor="inv-cat" className="field-label">Category</label>
              <select id="inv-cat" value={categoryFilter} onChange={(e) => setCategoryFilter(e.target.value)}>
                <option value="">All categories</option>
                {categories.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            </div>
            <div>
              <label htmlFor="inv-stock" className="field-label">Stock status</label>
              <select id="inv-stock" value={stockFilter} onChange={(e) => setStockFilter(e.target.value as StockFilter)}>
                <option value="ALL">All stock</option>
                <option value="AVAILABLE">Available now</option>
                <option value="MAINTENANCE">Has maintenance units</option>
                <option value="WAITLIST">Waitlist (0 available)</option>
              </select>
            </div>
          </div>

          {models === null && !error ? (
            <p className="muted" role="status">Loading inventory…</p>
          ) : filteredModels.length === 0 ? (
            <p className="muted small">No equipment models match these filters.</p>
          ) : (
            <div className="table-wrap">
              <table aria-label="Equipment inventory">
                <thead>
                  <tr>
                    <th scope="col">Equipment</th>
                    <th scope="col">Category</th>
                    <th scope="col">Stock</th>
                    <th scope="col">Status</th>
                    <th scope="col">Actions</th>
                  </tr>
                </thead>
                {filteredModels.map((m) => {
                  const pct = m.totalUnits ? Math.round((m.availableUnits / m.totalUnits) * 100) : 0;
                  const maintCount = m.units.filter((u) => u.status === "MAINTENANCE").length;
                  const loanCount = m.units.filter((u) => u.status === "RESERVED" || u.status === "COLLECTED").length;
                  const hasActiveLoan = loanCount > 0;
                  const isExpanded = expandedModelId === m.id;
                  const confirmingModel = confirmDelete?.kind === "model" && confirmDelete.id === m.id;
                  return (
                    <tbody key={m.id}>
                      <tr>
                        <td className="inv-model-cell">
                          <div className="inv-model">
                            {m.imageUrl ? (
                              <img
                                className="inv-thumb"
                                src={m.imageUrl}
                                alt={m.imageAlt || m.name}
                                width={56}
                                height={42}
                              />
                            ) : (
                              <span className="inv-thumb inv-thumb-empty" aria-hidden="true">
                                <Icon name={categoryIcon(m.category)} size={18} />
                              </span>
                            )}
                            <div className="inv-model-meta">
                              <strong className="block">{m.name}</strong>
                              <span className="tag">{m.id}</span>{" "}
                              <span className="muted small">{m.description} · Up to {m.loanDurationDays} days</span>
                            </div>
                          </div>
                        </td>
                        <td>
                          <span className="pill pill-plain">
                            <Icon name={categoryIcon(m.category)} size={14} /> {m.category}
                          </span>
                        </td>
                        <td>
                          <strong>{m.availableUnits}</strong> <span className="muted small">/ {m.totalUnits} available</span>
                          <span className={`bar${m.availableUnits === 0 ? " danger" : m.availableUnits === 1 ? " warn" : ""}`}>
                            <span style={{ width: `${pct}%` }} />
                          </span>
                          {(maintCount > 0 || loanCount > 0) && (
                            <span className="muted small block">
                              {maintCount > 0 && `${maintCount} maintenance`}
                              {maintCount > 0 && loanCount > 0 && " · "}
                              {loanCount > 0 && `${loanCount} on loan/reserved`}
                            </span>
                          )}
                        </td>
                        <td>
                          <span className={`pill ${m.availableUnits > 0 ? "pill-in" : "pill-out"}`}>
                            {m.availableUnits > 0 ? "Available" : "Waitlist"}
                          </span>
                        </td>
                        <td>
                          <div className="row-start">
                            <button
                              type="button"
                              className="btn"
                              aria-expanded={isExpanded}
                              aria-controls={`units-${m.id}`}
                              onClick={() => setExpandedModelId(isExpanded ? null : m.id)}
                            >
                              <Icon name="wrench" size={15} />
                              <span>Manage units ({m.units.length})</span>
                            </button>
                            {confirmingModel ? (
                              <span className="confirm" role="group" aria-label={`Confirm removal of ${m.name}`}>
                                <button
                                  type="button"
                                  className="btn danger"
                                  disabled={busyKey !== null}
                                  aria-busy={busyKey === `del-model:${m.id}`}
                                  onClick={() => handleRemoveModel(m)}
                                >
                                  {busyKey === `del-model:${m.id}` ? "Removing…" : "Confirm remove"}
                                </button>{" "}
                                <button
                                  type="button"
                                  className="btn"
                                  disabled={busyKey !== null}
                                  onClick={() => setConfirmDelete(null)}
                                >
                                  Cancel
                                </button>
                              </span>
                            ) : (
                              <button
                                type="button"
                                className="btn"
                                disabled={busyKey !== null || hasActiveLoan}
                                title={hasActiveLoan ? "Cannot remove model while units are reserved or collected" : `Remove ${m.name}`}
                                onClick={() => setConfirmDelete({ kind: "model", id: m.id })}
                              >
                                <Icon name="trash" size={15} />
                                <span>Remove model</span>
                              </button>
                            )}
                          </div>
                        </td>
                      </tr>
                      {isExpanded && (
                        <tr id={`units-${m.id}`} className="unit-drawer-row">
                          <td colSpan={5}>
                            <div className="unit-panel">
                              <div className="row row-wrap">
                                <strong>Physical units for {m.name}</strong>
                                <span className="muted small">{m.specifications?.length ?? 0} spec{(m.specifications?.length ?? 0) === 1 ? "" : "s"} recorded</span>
                              </div>
                              {m.units.length === 0 ? (
                                <p className="muted small">No physical units remain for this model. Add a unit below.</p>
                              ) : (
                                <ul className="unit-list">
                                  {m.units.map((u) => {
                                    const locked = u.status === "RESERVED" || u.status === "COLLECTED";
                                    const confirmingUnit = confirmDelete?.kind === "unit" && confirmDelete.id === u.unitId;
                                    const unitBusy = busyKey === u.unitId;
                                    return (
                                      <li key={u.unitId} className="unit-row">
                                        <div className="unit-info">
                                          <span className="tag">{u.unitId}</span>
                                          <span className="small">Serial: <strong>{u.serialNumber}</strong></span>
                                          <span className={`pill ${unitToneClass(u.status)}`}>{u.status}</span>
                                        </div>
                                        <div className="row-start">
                                          <button
                                            type="button"
                                            className="btn"
                                            disabled={busyKey !== null || locked}
                                            aria-busy={unitBusy}
                                            title={locked ? "Reserved or collected units cannot change maintenance status" : undefined}
                                            onClick={() => toggleMaintenance(m, u)}
                                          >
                                            <Icon name="wrench" size={15} />
                                            <span>{u.status === "MAINTENANCE" ? "Set available" : "Move to maintenance"}</span>
                                          </button>
                                          {confirmingUnit ? (
                                            <span className="confirm" role="group" aria-label={`Confirm removal of unit ${u.unitId}`}>
                                              <button
                                                type="button"
                                                className="btn danger"
                                                disabled={busyKey !== null}
                                                aria-busy={unitBusy}
                                                onClick={() => handleRemoveUnit(m, u)}
                                              >
                                                {unitBusy ? "Removing…" : "Confirm remove"}
                                              </button>{" "}
                                              <button
                                                type="button"
                                                className="btn"
                                                disabled={busyKey !== null}
                                                onClick={() => setConfirmDelete(null)}
                                              >
                                                Cancel
                                              </button>
                                            </span>
                                          ) : (
                                            <button
                                              type="button"
                                              className="btn"
                                              disabled={busyKey !== null || locked}
                                              title={locked ? "Cannot remove a unit that is reserved or out on loan" : `Remove unit ${u.unitId}`}
                                              onClick={() => setConfirmDelete({ kind: "unit", id: u.unitId })}
                                            >
                                              <Icon name="trash" size={15} />
                                              <span>Remove</span>
                                            </button>
                                          )}
                                        </div>
                                      </li>
                                    );
                                  })}
                                </ul>
                              )}
                              <form className="add-unit-form" onSubmit={(e) => handleAddUnit(e, m)}>
                                <div className="field">
                                  <label htmlFor={`add-serial-${m.id}`}>New unit serial number <span className="req" aria-hidden="true">*</span></label>
                                  <input
                                    id={`add-serial-${m.id}`}
                                    type="text"
                                    required
                                    placeholder={`e.g. ${defaultSerial(m.id, m.units.length)}`}
                                    value={unitAddSerial[m.id] ?? ""}
                                    onChange={(e) => setUnitAddSerial((p) => ({ ...p, [m.id]: e.target.value }))}
                                  />
                                </div>
                                <div className="field">
                                  <label htmlFor={`add-status-${m.id}`}>Initial status</label>
                                  <select
                                    id={`add-status-${m.id}`}
                                    value={unitAddStatus[m.id] ?? "AVAILABLE"}
                                    onChange={(e) =>
                                      setUnitAddStatus((p) => ({
                                        ...p,
                                        [m.id]: e.target.value as "AVAILABLE" | "MAINTENANCE",
                                      }))
                                    }
                                  >
                                    <option value="AVAILABLE">Available</option>
                                    <option value="MAINTENANCE">Maintenance</option>
                                  </select>
                                </div>
                                <button
                                  type="submit"
                                  className="btn primary"
                                  disabled={busyKey !== null}
                                  aria-busy={busyKey === `add:${m.id}`}
                                >
                                  <Icon name="plus" size={15} />
                                  <span>{busyKey === `add:${m.id}` ? "Adding…" : "Add unit"}</span>
                                </button>
                              </form>
                            </div>
                          </td>
                        </tr>
                      )}
                    </tbody>
                  );
                })}
              </table>
            </div>
          )}
        </section>
      </div>

      <dialog ref={dialogRef} className="modal glass" aria-labelledby="add-equipment-title">
        <div className="modal-head">
          <div>
            <span className="eyebrow">Inventory catalogue</span>
            <h2 id="add-equipment-title">Add new equipment</h2>
          </div>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close add equipment dialog"
            onClick={() => dialogRef.current?.close()}
          >
            ✕
          </button>
        </div>

        <p className="muted small">All fields are mandatory before saving a product to the live catalogue.</p>

        {formError && <div role="alert" className="alert">{formError}</div>}

        <form ref={formRef} onSubmit={handleCreateModel} noValidate className="modal-form">
          <div className="form-grid">
            <div className="field">
              <label htmlFor="inv-add-name">Equipment name <span className="req" aria-hidden="true">*</span></label>
              <input
                id="inv-add-name"
                name="name"
                type="text"
                required
                maxLength={160}
                placeholder="e.g. Sony FX3 Cinema Kit"
                value={name}
                onChange={(e) => updateName(e.target.value)}
                autoFocus
              />
            </div>

            <div className="field">
              <label htmlFor="inv-add-id">Model ID <span className="req" aria-hidden="true">*</span></label>
              <input
                id="inv-add-id"
                name="id"
                type="text"
                required
                minLength={2}
                maxLength={64}
                pattern="[a-z0-9]+(-[a-z0-9]+)*"
                autoCorrect="off"
                autoCapitalize="none"
                spellCheck={false}
                aria-describedby="inv-id-help"
                placeholder="e.g. sony-fx3-cinema-kit"
                value={modelId}
                onChange={(e) => updateModelId(e.target.value)}
              />
              <span id="inv-id-help" className="muted small">Lowercase letters, digits, and single hyphens.</span>
            </div>

            <div className="field">
              <label htmlFor="inv-add-category">Category <span className="req" aria-hidden="true">*</span></label>
              <input
                id="inv-add-category"
                name="category"
                type="text"
                list="inventory-category-list"
                required
                maxLength={80}
                placeholder="e.g. Camera"
                value={category}
                onChange={(e) => setCategory(e.target.value)}
              />
              <datalist id="inventory-category-list">
                {categoryOptions.map((c) => <option key={c} value={c} />)}
              </datalist>
            </div>

            <div className="field">
              <label htmlFor="inv-add-days">Maximum loan period (days) <span className="req" aria-hidden="true">*</span></label>
              <input
                id="inv-add-days"
                name="loanDurationDays"
                type="number"
                required
                min={1}
                max={14}
                step={1}
                value={loanDurationDays}
                onChange={(e) => setLoanDurationDays(e.target.value)}
              />
            </div>
          </div>

          <div className="field">
            <label htmlFor="inv-add-desc">Description <span className="req" aria-hidden="true">*</span></label>
            <textarea
              id="inv-add-desc"
              name="description"
              required
              rows={3}
              maxLength={1000}
              placeholder="Summarize key capabilities, included accessories, and intended coursework."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>

          <fieldset className="form-section">
            <legend>Product image <span className="req" aria-hidden="true">*</span></legend>
            <div className="radio-row">
              <label className="check">
                <input
                  type="radio"
                  name="imageMode"
                  checked={imageMode === "upload"}
                  onChange={() => {
                    setImageMode("upload");
                    setImageUrl("");
                  }}
                />
                <span>Upload image file (PNG, JPEG, WebP)</span>
              </label>
              <label className="check">
                <input
                  type="radio"
                  name="imageMode"
                  checked={imageMode === "url"}
                  onChange={() => {
                    setImageMode("url");
                    setImageUrl("");
                  }}
                />
                <span>Image URL or /catalogue/ path</span>
              </label>
            </div>

            <div className="form-grid">
              <div className="field">
                {imageMode === "upload" ? (
                  <>
                    <label htmlFor="inv-add-image">Choose product image <span className="req" aria-hidden="true">*</span></label>
                    <input
                      key="file"
                      id="inv-add-image"
                      name="imageFile"
                      type="file"
                      accept="image/png,image/jpeg,image/webp"
                      required={!imageUrl}
                      onChange={(e) => handleImageFile(e.target.files?.[0])}
                    />
                  </>
                ) : (
                  <>
                    <label htmlFor="inv-add-image">Image URL <span className="req" aria-hidden="true">*</span></label>
                    <input
                      key="url"
                      id="inv-add-image"
                      name="imageUrl"
                      type="text"
                      required
                      autoCorrect="off"
                      autoCapitalize="none"
                      spellCheck={false}
                      placeholder="https://... or /catalogue/canon-eos-r50.webp"
                      value={imageUrl}
                      onChange={(e) => setImageUrl(e.target.value)}
                    />
                  </>
                )}
              </div>

              <div className="field">
                <label htmlFor="inv-add-alt">Image alt text <span className="req" aria-hidden="true">*</span></label>
                <input
                  id="inv-add-alt"
                  name="imageAlt"
                  type="text"
                  required
                  maxLength={200}
                  placeholder="Describe the device photo for screen readers"
                  value={imageAlt}
                  onChange={(e) => {
                    setAltTouched(true);
                    setImageAlt(e.target.value);
                  }}
                />
              </div>
            </div>

            {imageUrl && (
              <div className="img-preview">
                <img src={imageUrl} alt={imageAlt || "Selected product preview"} />
              </div>
            )}
          </fieldset>

          <fieldset className="form-section">
            <legend>Specifications <span className="req" aria-hidden="true">*</span></legend>
            {specs.map((spec, idx) => (
              <div key={spec.key} className="spec-row">
                <div className="field">
                  <label htmlFor={`spec-label-${spec.key}`}>Label {idx + 1} <span className="req" aria-hidden="true">*</span></label>
                  <input
                    id={`spec-label-${spec.key}`}
                    type="text"
                    required
                    maxLength={120}
                    placeholder="e.g. Sensor"
                    value={spec.label}
                    onChange={(e) =>
                      setSpecs((cur) => cur.map((r) => (r.key === spec.key ? { ...r, label: e.target.value } : r)))
                    }
                  />
                </div>
                <div className="field">
                  <label htmlFor={`spec-value-${spec.key}`}>Value {idx + 1} <span className="req" aria-hidden="true">*</span></label>
                  <input
                    id={`spec-value-${spec.key}`}
                    type="text"
                    required
                    maxLength={240}
                    placeholder="e.g. Full-frame 12.1 MP"
                    value={spec.value}
                    onChange={(e) =>
                      setSpecs((cur) => cur.map((r) => (r.key === spec.key ? { ...r, value: e.target.value } : r)))
                    }
                  />
                </div>
                {specs.length > 1 && (
                  <button
                    type="button"
                    className="btn"
                    onClick={() => setSpecs((cur) => cur.filter((r) => r.key !== spec.key))}
                  >
                    Remove
                  </button>
                )}
              </div>
            ))}
            <button
              type="button"
              className="link-btn"
              onClick={() => {
                const key = ++nextRowKey.current;
                setSpecs((cur) => [...cur, { key, label: "", value: "" }]);
              }}
            >
              <Icon name="plus" size={14} /> Add specification
            </button>
          </fieldset>

          <fieldset className="form-section">
            <legend>Initial stock units <span className="req" aria-hidden="true">*</span></legend>
            {units.map((u, idx) => (
              <div key={u.key} className="unit-input-row">
                <div className="field">
                  <label htmlFor={`unit-serial-${u.key}`}>Serial number {idx + 1} <span className="req" aria-hidden="true">*</span></label>
                  <input
                    id={`unit-serial-${u.key}`}
                    type="text"
                    required
                    maxLength={120}
                    placeholder={`e.g. ${defaultSerial(modelId || "item", idx)}`}
                    value={u.serialNumber}
                    onChange={(e) =>
                      setUnits((cur) =>
                        cur.map((r) => (r.key === u.key ? { ...r, serialNumber: e.target.value, touched: true } : r))
                      )
                    }
                  />
                </div>
                <div className="field">
                  <label htmlFor={`unit-status-${u.key}`}>Status {idx + 1}</label>
                  <select
                    id={`unit-status-${u.key}`}
                    value={u.status}
                    onChange={(e) =>
                      setUnits((cur) =>
                        cur.map((r) =>
                          r.key === u.key ? { ...r, status: e.target.value as "AVAILABLE" | "MAINTENANCE" } : r
                        )
                      )
                    }
                  >
                    <option value="AVAILABLE">Available</option>
                    <option value="MAINTENANCE">Maintenance</option>
                  </select>
                </div>
                {units.length > 1 && (
                  <button
                    type="button"
                    className="btn"
                    onClick={() => setUnits((cur) => cur.filter((r) => r.key !== u.key))}
                  >
                    Remove
                  </button>
                )}
              </div>
            ))}
            <button
              type="button"
              className="link-btn"
              onClick={() => {
                const key = ++nextRowKey.current;
                setUnits((cur) => [
                  ...cur,
                  {
                    key,
                    serialNumber: modelId ? defaultSerial(slugify(modelId), cur.length) : "",
                    status: "AVAILABLE",
                    touched: false,
                  },
                ]);
              }}
            >
              <Icon name="plus" size={14} /> Add unit
            </button>
          </fieldset>

          <div className="modal-foot">
            <button type="button" className="btn" disabled={creating} onClick={() => dialogRef.current?.close()}>
              Cancel
            </button>
            <button type="submit" className="btn primary" disabled={creating} aria-busy={creating}>
              <Icon name="check" size={16} />
              <span>{creating ? "Saving equipment…" : "Save equipment"}</span>
            </button>
          </div>
        </form>
      </dialog>
    </div>
  );
}
