"use client";

import Link from "next/link";
import { useState } from "react";
import { Icon } from "./Icon";

export type StaffPage = "dashboard" | "inventory";

export function StaffSidebar({
  activePage,
  activeSection = "dashboard",
  onSelectSection,
  queueCount = 0,
  overdueCount = 0,
  inventoryCount,
}: {
  activePage: StaffPage;
  activeSection?: string;
  onSelectSection?: (key: string) => void;
  queueCount?: number;
  overdueCount?: number;
  inventoryCount?: number;
}) {
  const [dashboardOpen, setDashboardOpen] = useState(activePage === "dashboard");

  const sectionLinkProps = (key: string) => ({
    "aria-current": activePage === "dashboard" && activeSection === key ? ("location" as const) : undefined,
    onClick: () => {
      setDashboardOpen(true);
      onSelectSection?.(key);
    },
  });

  return (
    <aside className="admin-side glass" aria-label="Staff sections">
      <div className="side-head">
        <Icon name="shield" />
        <div>
          <strong className="block">Staff portal</strong>
          <span className="muted small">Resource Center desk</span>
        </div>
      </div>
      <ul className="staff-nav-list">
        <li className="tree-node">
          <div className={`tree-row${activePage === "dashboard" ? " is-active" : ""}`}>
            <Link
              href="/staff/dashboard"
              className="tree-link"
              aria-current={activePage === "dashboard" && activeSection === "dashboard" ? "page" : undefined}
              onClick={(e) => {
                if (activePage === "dashboard") {
                  e.preventDefault();
                  setDashboardOpen((open) => {
                    const next = !open;
                    if (next) onSelectSection?.("dashboard");
                    return next;
                  });
                } else {
                  setDashboardOpen(true);
                }
              }}
            >
              <Icon name="dashboard" />
              <span>Dashboard</span>
            </Link>
            <button
              type="button"
              className="tree-toggle"
              aria-expanded={dashboardOpen}
              aria-controls="staff-dashboard-tree"
              aria-label={dashboardOpen ? "Collapse dashboard sections" : "Expand dashboard sections"}
              onClick={() => setDashboardOpen((open) => !open)}
            >
              <Icon name="chevronDown" size={16} />
            </button>
          </div>
          <ul id="staff-dashboard-tree" className="tree-children" hidden={!dashboardOpen}>
            <li>
              <Link href="/staff/dashboard#queue" {...sectionLinkProps("queue")}>
                <Icon name="clock" size={16} />
                <span>Collection queue</span>
                {queueCount > 0 && <span className="count">{queueCount}</span>}
              </Link>
            </li>
            <li>
              <Link href="/staff/dashboard#loans" {...sectionLinkProps("loans")}>
                <Icon name="swap" size={16} />
                <span>Loans</span>
              </Link>
            </li>
            <li>
              <Link href="/staff/dashboard#overdue" {...sectionLinkProps("overdue")}>
                <Icon name="alert" size={16} />
                <span>Overdue</span>
                {overdueCount > 0 && <span className="count danger">{overdueCount}</span>}
              </Link>
            </li>
          </ul>
        </li>
        <li>
          <Link
            href="/staff/inventory"
            className="side-link"
            aria-current={activePage === "inventory" ? "page" : undefined}
          >
            <Icon name="box" />
            <span>Inventory</span>
            {typeof inventoryCount === "number" && <span className="count">{inventoryCount}</span>}
          </Link>
        </li>
      </ul>
    </aside>
  );
}
