# Submission preparation

Status: partial. No final report, media set or tutor sign-off was supplied for this
phase. No submission ZIP was made and nothing was sent to Blackboard. The stored
brief's deadline is not a verified current cohort deadline; check Blackboard.

## Report outline: 1,500–2,000 words

Write this in your own words. Use your own choices, evidence and conclusions.
The allocation below totals 1,800 words; it is an outline, not a written report.

| Section | Words | Questions and evidence |
| --- | ---: | --- |
| Scope and achieved MVP | 150 | What did you build? What did the tutor approve? Separate missing proof from working features. |
| Architecture and ownership | 300 | Explain the two services, Loan-owned outbox/publisher, ports, private databases and async flow. Use the C4 and Stage 2 diagrams. |
| Security | 250 | Explain Auth0, JWT checks, server token handling, roles, TLS and cloud identities. Cite real 403/login evidence only when captured. |
| Data and concurrency | 250 | Explain model-first locks, atomic outbox writes, two-day due dates, rollback and five-way contention. |
| Resilience and scaling | 250 | Explain leases, at-least-once processing, retry limits and the email send-before-record gap. Compare fake tests with actual service evidence. |
| Delivery and operations | 300 | Explain immutable digests, migration-first maintenance, seed safety, gates, environment isolation, monitoring and recovery. Label unrun workflows. |
| Costs, limits and reflection | 300 | Weigh fixed database cost, scale-to-zero limits, complexity and alternatives. State what you learned and what you would change. Do not invent success or tutor feedback. |

## Evidence to capture

Use full-resolution PNG/JPG screenshots and separate 20–30-second H.264 MP4 clips.
Keep all clips together below five minutes. Avoid visible secrets, tokens, student
emails and unrelated browser tabs.

| Clip or screenshot | Required proof | Current gap |
| --- | --- | --- |
| Catalogue and reservation | Available count drops, due date appears; competing request fails safely | Local SQL tests pass; live signed-in browser proof missing |
| Staff collection and return | Correct timestamps and availability; student gets 403 | Local tests pass; live role proof missing |
| Notifications | Collection, return and two waiting recipients receive email | Fake evidence exists; approved recipients and live email proof missing |
| Pipeline | Commit, checks, exact image digests, migration success, smoke and artifact | Workflow source/local tests only for phase 4 |
| Blocked then approved promotion | Independent reviewer protection and matching Test digests | Actual settings and runs unverified |
| Recovery and scaling | Outage with Loan commits, backlog drain, request latency/errors and replicas | Fake/local and older stored evidence; current live checks pending |
| Monitoring | One correlation across services, metrics and a supported alert test | Older checks; alert routing remains unproved |

Use [phase 4 handoff](../backend/phase-4-handoff.md) as the current results index,
[release runbook](../backend/release-runbook.md) for safe operations, and
[assessment map](../cloud/assessment-evidence.md) for diagrams. Preserve dated older
evidence; do not relabel it as a new run. Confirm Stage 2 approval with the tutor.

## Build the ZIP after final inputs exist

```bash
python3 scripts/package-submission.py --name 'SUPPLIED NAME' --user-id 'SUPPLIED ID' \
  --report /absolute/path/final-report.pdf --media /absolute/path/media \
  --output /absolute/path/submission.zip
```

The packager creates root `readme.txt`, `Source/`, `Documentation/`, and `Media/`.
It includes reviewed tracked and new non-ignored source, lockfiles, config examples
and docs. It excludes local env files, real tfvars, backend overrides, state,
plans, caches, dependency/build folders, symlinks and logs. Existing output ZIPs
cause an error. It requires a report and media, checks MP4 codec/clip length/total
duration, and refuses missing final inputs. It does not verify the report's word
count, image resolution, factual accuracy or tutor approval. Inspect the ZIP and
play every clip before submission. Keep only the final pack you need.
