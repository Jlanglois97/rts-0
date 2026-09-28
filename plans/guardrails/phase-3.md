# Phase 3: Remove the command/ability notice cycle

Status: Planned

## Goal and scope

Remove the existing `ability_orders -> commands` dependency by relocating the two shared notice
constructors. Preserve the exact notice payload, recipient, and emission order.

1. Move `notice` and `notice_positioned` from `services/commands.rs` to a small shared game event
   helper, preferably `game/notices.rs`. Keep it private to the game and limited to appending the
   existing Notice event to the supplied recipient accumulator.
2. Update commands, ability orders, and any remaining callers to import the helper directly. Do not
   leave a compatibility re-export from commands or introduce a generic event bus.
3. Remove the ability-to-commands entry from both the service dependency allowlist and role-edge
   exception list. Keep the existing unrelated ability-to-movement exception untouched.
4. Confirm no service code reaches the old notice API by an alternative path. Update baseline
   edges/exports and the design document where it describes the residual command back-edge.

## Expected touch points

- `server/crates/sim/src/game/notices.rs` (new), `game/mod.rs` module registration.
- `game/services/commands.rs`, `game/services/ability_orders.rs`, and actual helper callers.
- `server/crates/archcheck/src/lib.rs` and its baseline.
- `docs/design/server-sim.md` if its responsibility/exception description changes.

## Verification and acceptance

```bash
cargo test --manifest-path server/Cargo.toml -p rts-archcheck
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml -p rts-sim -E 'test(game::services::commands::tests)'
node scripts/check-docs-health.mjs
```

Inspect existing ability rejection/resource-notice cases and run additional existing tests if they
live outside the commands module. Preserve recipient ID, message text, coordinates, severity, and
append order; add a regression test only if a material behavior has no existing coverage.
The measured service graph must no longer contain `ability_orders -> commands`, and restoring that
edge must be rejected by the checker without changing its policy.

## Manual testing focus and handoff

Suggested smoke check, if a local game is available: trigger one resource rejection and one
positioned ability notice and confirm the expected message/marker for the issuing player. Automated
coverage is the acceptance gate for this mechanical move; report whether the smoke check ran.
Hand off the helper location, removed exceptions, actual tests, and merged PR/head to Phase 4.
