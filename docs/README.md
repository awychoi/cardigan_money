# Documentation

This folder is the single home for project documentation. The repository root keeps only
[README.md](../README.md) and [SECURITY.md](../SECURITY.md); everything else lives in one of the
groups below.

## Start here

| I want to… | Read |
|---|---|
| see the completed UI overhaul | [plans/ui-overhaul-2026-10.md](plans/ui-overhaul-2026-10.md) |
| know what is actually built and verified | [reference/implementation-status.md](reference/implementation-status.md) |
| know why a technical choice was made | [reference/decisions.md](reference/decisions.md) |
| understand the security posture | [SECURITY.md](../SECURITY.md), [security/architecture.md](security/architecture.md), [security/threat-model.md](security/threat-model.md) |
| change how the app looks | [design/theming.md](design/theming.md) |
| see the UI direction | [design/proposal.md](design/proposal.md) |

## Map

| Folder | Contains |
|---|---|
| [`plans/`](plans) | The one live plan, plus archived plans |
| [`security/`](security) | Architecture, threat model, incident runbook, dated reviews |
| [`design/`](design) | UI proposal, interactive mockup, theming reference, screenshots |
| [`operations/`](operations) | Backup/recovery, performance, native smoke, adversarial verification |
| [`reference/`](reference) | Decisions, data dictionary, test plan, implementation status |
| [`integrations/`](integrations) | Plaid feasibility, setup guide, verification records |

**Plans**

- [plans/ui-overhaul-2026-10.md](plans/ui-overhaul-2026-10.md) — completed: charts, Overview, Transactions, Settings
- [plans/archive/2026-10-initial-build-plan.md](plans/archive/2026-10-initial-build-plan.md) — archived

**Security**

- [security/architecture.md](security/architecture.md) — how the controls are built
- [security/threat-model.md](security/threat-model.md) — assets, adversaries, mitigations
- [security/incident-runbook.md](security/incident-runbook.md) — what to do if something breaks
- [security/design-review-2026-10-23.md](security/design-review-2026-10-23.md) — pre-implementation review (historical)
- [security/independent-review-2026-10-23.md](security/independent-review-2026-10-23.md) — post-implementation review and dispositions

**Design**

- [design/theming.md](design/theming.md) — theme schema, authoring guide, limits
- [design/proposal.md](design/proposal.md) — the UI direction and what shipped from it
- [design/mockup.html](design/mockup.html) — clickable mockup (Overview, Cards, Settings, language sheet)

**Operations**

- [operations/backup-recovery.md](operations/backup-recovery.md)
- [operations/native-smoke.md](operations/native-smoke.md)
- [operations/performance.md](operations/performance.md)
- [operations/adversarial-verification.md](operations/adversarial-verification.md)

**Reference**

- [reference/decisions.md](reference/decisions.md) — owner, security, and implementation decisions
- [reference/data-dictionary.md](reference/data-dictionary.md)
- [reference/test-plan.md](reference/test-plan.md)
- [reference/implementation-status.md](reference/implementation-status.md)

**Integrations**

- [integrations/plaid-feasibility.md](integrations/plaid-feasibility.md)
- [integrations/plaid-guide.md](integrations/plaid-guide.md)
- [integrations/plaid-verification-2026-10-24.md](integrations/plaid-verification-2026-10-24.md)

## House rules

1. **Two markdown files at the root.** New documents go into a group above, never the root.
2. **One live plan.** When a plan is superseded, move it to `plans/archive/`, rename it with its
   month, and add an `Archived` banner at the top. The current plan is the only source of intent.
3. **Security reviews are dated and immutable.** Do not edit findings; record dispositions in the
   decision log and, if needed, a new dated review.
4. **Decisions are append-only.** Owner and security decisions are normative; the implementation
   log records choices made inside them. Superseded decisions are marked, not deleted.
5. **Status is evidence, not aspiration.** Update `reference/implementation-status.md` at the end
   of every phase with what ran and what did not.
6. **Retire, don't leave stale.** Remove superseded documents instead of parking them next to
   current ones. Record them below with the commit that still contains them.

## Retired documents

| Document | Retired | Recovery |
|---|---|---|
| Build specification (2026-10, 104 KB) | 2026-10-24 — owner and security decision tables extracted into [reference/decisions.md](reference/decisions.md) | `git show b05396b:personal-cards-build-spec.md` |
| Root-level implementation plan | 2026-10-24 — archived, not deleted | [plans/archive/2026-10-initial-build-plan.md](plans/archive/2026-10-initial-build-plan.md) |
| Root-level security review | 2026-10-24 — moved into the security group | [security/design-review-2026-10-23.md](security/design-review-2026-10-23.md) |
| Flat `docs/*.md` layout | 2026-10-24 — regrouped into plans, security, operations, reference, integrations | This file |
