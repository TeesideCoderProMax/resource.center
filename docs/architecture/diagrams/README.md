# Architecture diagrams

`resource-center-c4.drawio` is the editable source (draw.io C4 shape library). Pages:

| Page | PNG | Brief stage |
| --- | --- | --- |
| 1 Context | `1-context.png` | Stage 1, C4 Context (required) |
| 2 Container | `2-container.png` | Stage 1, C4 Container (required) |
| 3 Deployment | `3-deployment.png` | Stage 1, Deployment (recommended) |
| 4 Stage 2 Plan | `4-stage2-plan.png` | Stage 2: copy of the Container diagram with requirements, security, fakes, MVP and build order |

After editing, re-export each page from draw.io with File → Export as → PNG, zoom 200%, white background, and save it over the matching PNG. Values on the Deployment page come from `infra/platform` and `infra/workloads`; re-check them there after infrastructure changes.
