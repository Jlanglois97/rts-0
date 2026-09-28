# Phase 1: Restore snapshot-call enforcement

Status: Planned

## Goal and scope

Close the mismatch between current public Game snapshot APIs and the lobby architecture checker.
Keep runtime projection and recipient policy unchanged.

1. Inventory public snapshot entry points in `game/snapshot.rs` and `game/observer_snapshot.rs`,
   along with their production lobby call sites. Cover player, spectator, observer, and full-world
   methods, including all current `_with_options` variants.
2. Extend `scripts/check-lobby-architecture.mjs` so these methods require the existing projection
   boundary. Retain only a demonstrated, narrowly named AI exception if a current caller needs it;
   an AI exception must never permit omniscient snapshots.
3. Expose the checker logic to a small fixture harness without executing the production scan when
   imported. Keep CLI success/failure behavior intact and preserve existing lab and size checks.
4. Test each current entry point from an unauthorized file and permitted projection file. Include
   multiline calls and the existing test-module exclusion behavior, and ensure similarly named
   projection-policy helpers are not mistaken for Game snapshot calls.
5. Add an API-inventory assertion so adding a public snapshot entry point requires an explicit
   policy decision. Run the fixtures in the existing architecture-policy gate and ensure relevant
   script changes select that gate; do not add a separate workflow.
6. Update the design document's lobby enforcement description to reflect actual coverage and limits.

## Expected touch points

- `scripts/check-lobby-architecture.mjs` and a focused adjacent fixture test file.
- `tests/run-all.sh` architecture-policy integration and existing test selection only if needed.
- `server/crates/sim/src/game/{snapshot,observer_snapshot}.rs` as read-only API inventory inputs.
- `server/src/lobby/projection.rs` as a read-only production caller.
- `docs/design/server-sim.md` lobby guardrail description.

## Verification and acceptance

- Run the new Node fixture harness and `node scripts/check-lobby-architecture.mjs`.
- Run `node scripts/check-docs-health.mjs`; validate any changed test selection with its existing
  self-check command.
- Demonstrate that a fixture calling `snapshot_for_observer_with_options` outside projection fails,
  including when the observer is omniscient. All current production callers must pass.
- The public API inventory must have no silently uncovered snapshot methods.

## Manual testing focus and handoff

No gameplay smoke test is required for checker-only changes. Inspect one forbidden-call diagnostic
for a useful file/method location, then hand off the method inventory, fixture command, gate wiring,
remaining syntax limitations, and merged PR/head to Phase 2.
