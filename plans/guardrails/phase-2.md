# Phase 2: Detect service dependencies expressed as calls

Status: Planned

## Goal and scope

Make dependency enforcement recognize the concrete service path forms already present in the sim.
Preserve the current role matrix and avoid a general Rust semantic-analysis project.

1. Extend service-edge collection beyond `use` statements to qualified paths in calls and type
   references. At minimum cover `crate::game::services::<service>::...`, including the real
   construction-to-world-query call, without treating comments or strings as edges.
2. Inventory relative imports, grouped imports, aliases, and re-exports used in service files.
   Resolve syntactic paths where their module target is known; aliases must retain their original
   target. Support relevant `super` paths with the correct parent module. Document unsupported
   macro-generated or semantic indirection rather than claiming complete Rust name resolution.
3. Reuse the current role/allowlist validation for the collected edges. Deduplicate by source,
   target, and file so repeated calls do not inflate dependency counts or baseline churn.
4. Add fixtures for allowed and forbidden direct calls, qualified types, representative relative
   and aliased imports, grouped imports, repeated references, and comments/string/test exclusions.
   A forbidden role edge must fail whether written as an import or a direct call.
5. Review the newly exposed production edge inventory. The known construction-to-world-query edge
   is a tick-system-to-query dependency and may receive an exact allowlist entry with justification.
   Do not grant a general service exemption; report any newly discovered forbidden-role edge that
   would require material work outside this phase.
6. Update only affected baseline measurements and document the scanner's actual coverage in the
   simulation design document. Existing pure-policy and mutable-state checks must keep passing.

A small syntax-parser dependency confined to archcheck is acceptable if it simplifies robust path
handling; a compiler plugin, macro expansion engine, or broad checker rewrite is outside scope.

## Expected touch points

- `server/crates/archcheck/src/lib.rs`, focused scanner helpers/tests, and its manifest if necessary.
- `server/crates/archcheck/baselines/sim-architecture.json` and Cargo.lock if needed.
- `docs/design/server-sim.md` ownership guardrails section.
- Production service code only for a narrowly justified dependency repair; ordinary behavior stays
  unchanged and any unexpected larger repair is a reported scope decision.

## Verification and acceptance

```bash
cargo test --manifest-path server/Cargo.toml -p rts-archcheck
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
node scripts/check-crate-boundaries.mjs
node scripts/check-docs-health.mjs
```

The construction-to-world-query dependency must appear in measured edges. Fixture tests must reject
forbidden calls with no `use` statement, and detect the representative relative/alias forms found
in the inventory. Review the baseline diff and report every newly exposed production dependency.

## Manual testing focus and handoff

No gameplay smoke test is required for analysis-only changes. Inspect allowed and forbidden fixture
diagnostics, then hand off supported syntax, limitations, justified edges, and merged PR/head.
Phase 3 should use the improved checker to prove removal of the ability-to-command back-edge.
