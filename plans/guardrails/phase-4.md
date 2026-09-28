# Phase 4: Give artillery tick execution its own owner

Status: Planned

## Goal and scope

Extract `artillery_point_fire_system` from the command adapter while preserving point-fire and
blanket-fire progression exactly. This is a responsibility move, not a redesign of artillery.

1. Move the ongoing system into a focused `services/artillery_execution.rs` tick-system module.
   Keep admission/issuing logic in commands and retain the existing `artillery_fire` mutation helper
   and `order_execution::targeting` logic. Do not change their roles merely to accommodate the move.
2. Update `systems.rs` to call the new owner at the same position: after combat, before economy.
   Preserve the `artillery_point_fire` perf label, argument values, entity iteration order, target
   validation, setup waiting behavior, invalid-order cleanup, reload/cost handling, and event order.
3. Register the new module as a TickSystem with only its actual query/mutation-helper dependencies.
   It must not import the commands adapter. Remove now-unused command imports and update only the
   relevant baseline entries; the relocated broad signature should replace the old one.
4. Update the simulation design document's service responsibility pointers. Keep Game API, durable
   state, checkpoint schema, wire protocol, fog policy, and balance unchanged.

## Expected touch points

- `server/crates/sim/src/game/services/{commands.rs,mod.rs,artillery_execution.rs}`.
- `server/crates/sim/src/game/systems.rs` call-site replacement.
- Existing artillery tests where their module paths require adjustment.
- `server/crates/archcheck/src/lib.rs`, its baseline, and `docs/design/server-sim.md`.

## Verification and acceptance

```bash
cargo test --manifest-path server/Cargo.toml -p rts-archcheck
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml -p rts-sim -E 'test(artillery)'
node scripts/check-crate-boundaries.mjs
node scripts/check-docs-health.mjs
```

Run the existing focused artillery cases before extraction and after it. Confirm coverage includes
immediate and queued point fire, blanket fire, setup/redeploy waiting, invalid target/order cleanup,
resource/reload constraints, and fog-filtered events; use additional existing named tests if their
names do not match the filter. Add a targeted regression only for an uncovered material guarantee.
Review the extraction diff to verify the function body and orchestration order are unchanged except
for necessary imports/paths. A new gameplay rule or replay difference blocks acceptance.

## Manual testing focus and handoff

Suggested smoke check, if a local game is available: queue move then fire on packed artillery and
exercise blanket fire after deployment. Check setup, movement handoff, firing, and resource use;
report whether this was actually run rather than implying manual evidence.
After the PR merges, hand off the new tick owner, unchanged tick position, actual verification, and
any remaining limitations. Mark this phase Done in its implementation commit so the PR helper can
archive the completed plan; the final handoff should point to the archived entry and leave deferred
architecture work unimplemented.
