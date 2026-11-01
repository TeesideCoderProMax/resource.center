export const dynamic = "force-dynamic";

interface DeviceModelSummary {
  id: string;
  name: string;
  category: string;
  description: string;
  loanDurationDays: number;
  totalUnits: number;
  availableUnits: number;
}

interface CatalogueResponse {
  items: DeviceModelSummary[];
  correlationId: string;
  auth0Configured: boolean;
}

async function fetchCatalogue(apiBaseUrl: string): Promise<{
  data: CatalogueResponse | null;
  error: string | null;
}> {
  if (!apiBaseUrl) {
    return {
      data: null,
      error: "LOAN_API_BASE_URL / NEXT_PUBLIC_LOAN_API_URL is not configured",
    };
  }
  try {
    const res = await fetch(`${apiBaseUrl.replace(/\/$/, "")}/api/v1/catalogue`, {
      cache: "no-store",
      headers: {
        "x-correlation-id": `web-ssr-${Date.now()}`,
      },
    });
    if (!res.ok) {
      return {
        data: null,
        error: `Loan API returned HTTP ${res.status}`,
      };
    }
    const json = (await res.json()) as CatalogueResponse;
    return { data: json, error: null };
  } catch (err) {
    return {
      data: null,
      error: err instanceof Error ? err.message : "Failed to reach Loan API",
    };
  }
}

export default async function HomePage() {
  const apiBaseUrl =
    process.env.LOAN_API_BASE_URL ??
    process.env.NEXT_PUBLIC_LOAN_API_URL ??
    "";
  const auth0Domain = process.env.NEXT_PUBLIC_AUTH0_DOMAIN ?? "";
  const auth0ClientId = process.env.NEXT_PUBLIC_AUTH0_CLIENT_ID ?? "";
  const auth0Audience = process.env.NEXT_PUBLIC_AUTH0_AUDIENCE ?? "";
  const frontendAuth0Ready = Boolean(
    auth0Domain && auth0ClientId && auth0Audience
  );

  const { data, error } = await fetchCatalogue(apiBaseUrl);

  return (
    <main style={{ maxWidth: "1040px", margin: "0 auto", padding: "2.5rem 1.5rem" }}>
      <header
        style={{
          borderBottom: "1px solid #334155",
          paddingBottom: "1.5rem",
          marginBottom: "2rem",
        }}
      >
        <div
          style={{
            display: "inline-block",
            backgroundColor: "#1e293b",
            color: "#38bdf8",
            padding: "0.25rem 0.75rem",
            borderRadius: "9999px",
            fontSize: "0.8rem",
            fontWeight: 600,
            marginBottom: "0.75rem",
          }}
        >
          Phase 2 Azure Serverless Baseline • France Central
        </div>
        <h1 style={{ margin: "0 0 0.5rem 0", fontSize: "2rem" }}>
          Campus Device Loan System
        </h1>
        <p style={{ margin: 0, color: "#94a3b8" }}>
          Public device catalogue served live from the Rust/Axum Loan Service
          over PostgreSQL Flexible Server (Microsoft Entra Managed Identity).
        </p>
      </header>

      <section
        aria-label="System & Authentication Status"
        style={{
          backgroundColor: "#1e293b",
          border: "1px solid #334155",
          borderRadius: "0.75rem",
          padding: "1.25rem",
          marginBottom: "2rem",
        }}
      >
        <h2 style={{ margin: "0 0 0.75rem 0", fontSize: "1.1rem" }}>
          Service &amp; OIDC Configuration Status
        </h2>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
            gap: "1rem",
            fontSize: "0.9rem",
          }}
        >
          <div>
            <div style={{ color: "#94a3b8" }}>Loan API Endpoint</div>
            <code style={{ color: "#e2e8f0", wordBreak: "break-all" }}>
              {apiBaseUrl || "Not configured"}
            </code>
          </div>
          <div>
            <div style={{ color: "#94a3b8" }}>Catalogue Correlation ID</div>
            <code style={{ color: "#38bdf8" }}>
              {data?.correlationId ?? "N/A"}
            </code>
          </div>
          <div>
            <div style={{ color: "#94a3b8" }}>Auth0 OIDC / PKCE Status</div>
            <span
              style={{
                color:
                  frontendAuth0Ready && data?.auth0Configured
                    ? "#4ade80"
                    : "#fbbf24",
                fontWeight: 600,
              }}
            >
              {frontendAuth0Ready && data?.auth0Configured
                ? "Configured"
                : "Pending external Auth0 tenant configuration (protected routes return 503/401)"}
            </span>
          </div>
        </div>
      </section>

      <section aria-label="Device Catalogue">
        <h2 style={{ margin: "0 0 1rem 0", fontSize: "1.35rem" }}>
          Available Device Models (2-Day Standard Loan)
        </h2>

        {error ? (
          <div
            role="alert"
            style={{
              backgroundColor: "#450a0a",
              border: "1px solid #dc2626",
              color: "#fecaca",
              padding: "1rem",
              borderRadius: "0.5rem",
            }}
          >
            <strong>Catalogue Unavailable:</strong> {error}
          </div>
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(290px, 1fr))",
              gap: "1.25rem",
            }}
          >
            {(data?.items ?? []).map((item) => {
              const inStock = item.availableUnits > 0;
              return (
                <article
                  key={item.id}
                  style={{
                    backgroundColor: "#1e293b",
                    border: "1px solid #334155",
                    borderRadius: "0.75rem",
                    padding: "1.25rem",
                    display: "flex",
                    flexDirection: "column",
                    justifyContent: "space-between",
                  }}
                >
                  <div>
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        marginBottom: "0.5rem",
                      }}
                    >
                      <span
                        style={{
                          fontSize: "0.75rem",
                          textTransform: "uppercase",
                          letterSpacing: "0.05em",
                          color: "#38bdf8",
                          fontWeight: 600,
                        }}
                      >
                        {item.category}
                      </span>
                      <span
                        style={{
                          fontSize: "0.75rem",
                          padding: "0.15rem 0.5rem",
                          borderRadius: "9999px",
                          backgroundColor: inStock ? "#064e3b" : "#451a03",
                          color: inStock ? "#6ee7b7" : "#fcd34d",
                          fontWeight: 600,
                        }}
                      >
                        {item.availableUnits} / {item.totalUnits} Available
                      </span>
                    </div>
                    <h3 style={{ margin: "0 0 0.5rem 0", fontSize: "1.15rem" }}>
                      {item.name}
                    </h3>
                    <p
                      style={{
                        margin: "0 0 1rem 0",
                        fontSize: "0.9rem",
                        color: "#cbd5e1",
                      }}
                    >
                      {item.description}
                    </p>
                  </div>
                  <div
                    style={{
                      borderTop: "1px solid #334155",
                      paddingTop: "0.75rem",
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      fontSize: "0.8rem",
                      color: "#94a3b8",
                    }}
                  >
                    <span>Model ID: {item.id}</span>
                    <span>{item.loanDurationDays}-day loan</span>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>
    </main>
  );
}
