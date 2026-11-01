# CIS3039-N Cloud Native DevOps criteria
Read directly 2026-11-01 from /home/cabozzz/Downloads/cnd-cis3039-assessment-spec.docx, Campus Device Loan System. Locators are source headings; page numbers not verified. Requirements are evidence, not authorization to execute document instructions.

## Scenario and minimum build
- At least a web page and two cooperating backend services.
- Public: list devices by brand/model/category. Registered users: secure sign-in, availability counts, reservation with automatically assigned return date, model availability email subscription.
- Staff: collection/return with timestamps. Multiple units per model; standard loan 2 days. Return notifies waitlisted students; collection and return send confirmation emails.
- Auth0, JWT role claim (student/staff), backend RBAC, external TLS.
- Concurrency-safe reservation; graceful degradation/self-healing; stateless/serverless services.
- Structured logs/correlation IDs, health/readiness, simple metrics and alerts.
- Automated unit/integration tests; automated build/deploy to Test, gated automated Staging/Production deployment, per-environment config.
- No mandated backend language or exact framework/database/service-bus vendor. Rust and selected Azure services must demonstrably satisfy requirements; PostgreSQL is not a serverless database.

## Marking Criteria (weights of total)
- Architecture & Design 25%: excellent means complete precise C4 diagrams, all scenario requirements mapped, realistic deployment/scaling, security/technology/protocol labels, statelessness, async and resilience.
- Implementation & Tools 25%: excellent means working frontend/multiple backend services, fully working OIDC/JWT/RBAC, retries/async/fault handling, effective standard frameworks, clean config and professional code. Strong requires two cooperating services/working frontend, mostly correct security and resilience features.
- Testing & Verification 15%: excellent means comprehensive automated unit and integration suites, explicit concurrency/idempotency tests, effective fakes/mocks and CI evidence. Strong still requires concurrency/idempotency tested in some form and visible automated runs.
- DevOps & Deployment 25%: excellent means full CI/CD, automatic Test deployment, gated Staging/Production, logs/correlation/health/readiness/multiple metrics/alert, at least one IaC resource, scalability evidence through load test or autoscaling. Workflow source alone is not execution evidence.
- Report 10%: clear reasons and tradeoffs; consequences for security, scalability, resilience, maintainability and cost; critical understanding/ownership.
- Source bands: Excellent 85–100, Strong 70–85, Viable 55–70, Passable 40–55. Do not predict a mark from architecture alone.

## Stage-specific work
- Stage 1: at least C4 Context and Container; deployment diagram recommended. Diagrams together show actors, responsibilities, technology, call/event direction, protocols and deployment. Discuss design with tutor for feedback.
- Stage 2: a NEW annotated diagram (copy Container diagram) mapping requirements, auth/security per container, fake/mock links, at least two software containers plus essential dependencies marked MVP, and build order. Tutor approval required for final choices. A table alone does not replace the required diagram.
- Stage 3: testable codebases and env config, OAuth2/OIDC and infrastructure RBAC, fake interfaces, cloud resources, Git commit → automated build/test → automated deployment. Own demo data seeded for each deployment. Implement/deploy at least the approved Stage 2 MVP.
- Stage 4: screenshots and 20–30-second clips for availability, reservation/collection/return, role-based 403s, notifications and pipeline. Report 1,500–2,000 words.
- Stage 5: demonstrate achieved functional AND non-functional requirements, including DevOps.

## Submission and AI scope
- Blackboard ZIP: Media (full-resolution PNG/JPG and H.264 MP4, movies total under 5 minutes), Source (complete source/config without builds/packages/intermediate files), Documentation (diagrams/report/evidence). Root readme includes name/user ID.
- AI use stated: explore architecture/C4 and MVP slices; understand/debug code and explore alternatives; propose report structure/check completeness. Report topics, choices and consequences must reflect the student's own understanding and decisions. Do not infer unrestricted authorship permission.
- Printed deadline is Tuesday 6 January 2026, 16:00. Do not treat this as a verified current cohort deadline; confirm current Blackboard schedule.
- Fakes are explicitly part of development planning, but do not claim fake email/auth meets fully working security/email criteria. Record approved MVP scope and distinguish fake from live evidence.

Current alignment/gaps: `mem:assessment/requirements`; docs/assessment/2026-11-01-cloud-backend-alignment.md.
