# Server guardrail repairs and bounded refactors

Status: Planned

## Objective and evidence

Restore enforcement of existing server boundaries and remove two concrete responsibility leaks.
This plan covers four small to medium changes from the server architecture review; it does not
implement gameplay changes. Creating this plan does not execute its phases.

Evidence was checked at `15afd9e2d992fd9ceff03882ae82235228bd9fc9` on 2026-09-28:

- The lobby checker matches three older snapshot method names, while current projection uses
  `snapshot_for_with_options` and `snapshot_for_observer_with_options`.
- The sim checker collects specific absolute `use` statements, missing fully qualified calls.
  `services/construction.rs` calls `world_query::completed_building_kinds`, although its service
  allowlist contains only occupancy and standability.
- `ability_orders` imports `notice` and `notice_positioned` from `commands`, creating an explicitly
  allowed back-edge into its caller.
- `commands.rs` owns `artillery_point_fire_system`, which `systems.rs` invokes every tick after
  combat and before economy.
- The crate, lobby, and sim architecture checks passed, as did all 32 archcheck tests. Passing
  therefore does not establish coverage of the identified gaps.
- A current HEAD hotspot report using the documented 14-day method ranked `commands.rs` as the
  highest server hotspot (repository rank 2; 2,046 nonempty lines and 232 historical touches).
  These counts are triage evidence, not a target for arbitrary file splitting.

## Phases

### Phase 1 — Restore snapshot-call enforcement

[Phase 1](phase-1.md) makes the lobby checker recognize every current public Game snapshot entry
point. It adds small negative fixtures for unauthorized callers and positive fixtures for the
projection boundary. Completion means the APIs used today are protected by an executable rule.

### Phase 2 — Detect service dependencies expressed as calls

[Phase 2](phase-2.md) extends the existing simulation checker to recognize direct qualified paths
and the import forms used in this repository. It accounts for newly exposed edges explicitly and
keeps the current service-role restrictions. Completion means spelling a dependency as a call
instead of an import no longer avoids the check.

### Phase 3 — Remove the command/ability notice cycle

[Phase 3](phase-3.md) moves the two notice constructors into a small shared event helper below the
command adapter. Commands and abilities use that helper with unchanged recipients and payloads.
Completion removes the ability-to-command dependency and its architectural exception.

### Phase 4 — Give artillery tick execution its own owner

[Phase 4](phase-4.md) extracts ongoing point/blanket-fire progression from command admission into a
focused tick-system module. The orchestrator calls it at exactly the existing point in the tick.
Completion preserves artillery behavior while leaving command handling responsible for issuing orders.

## Constraints across all phases

- Server-only scope; preserve gameplay, fog and event recipients, wire formats, balance, public
  Game signatures, command ordering, replay semantics, and timing.
- Keep the single room owner, explicit tick pipeline, and current crate dependency direction.
- Prefer the smallest checker extension covering real syntax in the repository. Do not build a
  general Rust name resolver, introduce a new lint framework, or redesign the entire checker.
- Negative fixtures must prove the intended violation is rejected; a test that only runs the
  checker on today's passing tree is insufficient. Document remaining scanner limitations.
- Review each newly detected dependency. Do not broadly allow new role edges just to turn CI green.
  If detection reveals an unrelated redesign requirement, report the exact edge and decision needed.
- Baseline changes must reflect only this phase's new measurements, moved functions, and removed
  exceptions. Inspect any `--bless` diff for unrelated growth; retain hard role restrictions.
- Read the simulation and testing capsules before implementation. Update the relevant sections of
  `docs/design/server-sim.md` when enforcement or service responsibilities change, and refresh the
  capsule only if its pointers change.
- Prefer existing behavior tests for mechanical extractions; add regression cases only for a
  material missing guarantee. No new performance benchmark or visual capture is required.
- No player-facing patch note is expected because behavior remains unchanged.

## Delivery and handoffs

Execute phases serially, starting each from current `origin/main` after the previous phase merges.
Use the phase-runner skill for executor passes. Implement and commit each phase on its own clean
`zvorygin/*` task branch, marking its phase document `Status: Done` in that implementation commit.
Push each as an owned PR with auto-merge armed through
`scripts/agent-pr.sh --verification "focused check command(s) passed"`, then run
`scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main` before reporting
completion or starting the next phase. GitHub's Main test gate remains the full-suite authority.
The final phase's helper will archive this plan when all phase documents are done.

After each phase, provide a handoff message with the merged PR/head, changes, actual verification,
remaining limitations, what the next agent should do, and the core features to manually test.
Manual notes should stay focused; report whether a suggested smoke check was actually performed.

## Deferred work

Complete order-transition ownership, entity-state privatization, room phase/state restructuring,
projection-context redesign, generalized fuzzing, and selective-baseline tooling remain outside this
plan. Reconsider a single order-family consolidation after these bounded changes land and its
cleanup obligations have been inventoried.
