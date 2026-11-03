import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { ApiClientError, errorHint } from "@/lib/api-client";
import { Catalogue } from "./Catalogue";
import { errorNotice, Notice } from "./Notice";

const err = (status: number, code?: string) => new ApiClientError("m", status, code, "web-1");

test("error hints map status and code", () => {
  expect(errorHint(err(401, "Unauthorized"))).toBe("sign-in");
  expect(errorHint(err(403, "ForbiddenRole"))).toBe("forbidden");
  expect(errorHint(err(403, "CrossOriginRejected"))).toBeNull();
  expect(errorHint(err(409, "NoAvailableUnits"))).toBe("subscribe");
  expect(errorHint(err(409, "InvalidTransition"))).toBe("refresh");
  expect(errorHint(err(503, "AuthUnavailable"))).toBe("degraded");
  expect(errorHint(err(502, "LoanApiUnreachable"))).toBe("degraded");
  expect(errorHint(new Error("x"))).toBeNull();
});

test("503 shows a degraded banner with the correlation ref", () => {
  render(<Notice notice={errorNotice(err(503, "AuthUnavailable"))} />);
  expect(screen.getByRole("alert")).toHaveTextContent("Service degraded. m (ref web-1)");
});

test("catalogue still renders its items under a degraded banner", () => {
  const items = [{ id: "a", name: "Laptop A", category: "laptop", description: "", loanDurationDays: 2, totalUnits: 1, availableUnits: 1 }];
  render(<Catalogue initialItems={items} initialError="Loan API returned HTTP 503" canAct={false} />);
  expect(screen.getByRole("alert")).toHaveTextContent("Service degraded.");
  expect(screen.getByText("Laptop A")).toBeInTheDocument();
});
