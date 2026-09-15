# jito-tip-router-pocs

Research artifact for the Jito Tip Router "unbounded weight parameters" case.

**Status: dropped. Not submitted to any bounty platform. Kept as a reference.**

## What this case was

Two weight parameters in the Jito Tip Router accept values without upper
bounds:

- `no_feed_weight: u128` — scales a vault's stake weight when no Switchboard
  oracle feed is configured (`core/src/vault_registry.rs`, `check_st_mint_entry`).
- `reward_multiplier_bps: u64` — multiplies a vault's reward share.

The original report (June 12, 2026) argued that these, combined with
permissionless `switchboard_set_weight` / epoch initialization instructions,
enable an automated reward-redistribution chain. See
`reports/JITO_TIP_ROUTER_FULL_REPORT.md`.

## Why it was dropped

Re-verification on Sep 15, 2026 (`CRANK_HUNT_NOTES.md` in the parent
workspace):

- `AdminRegisterStMint` / `AdminSetStMint` require an NCN admin signer
  (`core/src/instruction.rs:478-530`). Both parameters are admin-set and
  admin-fixable. Under Immunefi rules, "admin can set a bad parameter" is not
  a reportable finding.
- `SwitchboardSetWeight` is permissionless but only writes oracle-derived
  values: the registered feed key is enforced and a staleness check
  (`SWITCHBOARD_MAX_STALE_SLOTS = 100`) exists in
  `program/src/switchboard_set_weight.rs`. No attacker-controlled value path.
- The "no staleness check" observation on the `no_feed` path is by design:
  a constant weight has nothing to go stale.

The report in `reports/` is preserved as written; its permissionless-chain
claims are superseded by the re-verification above.

## Contents

- `reports/JITO_TIP_ROUTER_FULL_REPORT.md` — the original June 12, 2026 report
  (prepared for Immunefi, never submitted).
- `patches/0001-weight-bounds-research.patch` — WIP patch adding
  `MAX_NO_FEED_WEIGHT` and `MAX_REWARD_MULTIPLIER_BPS` bounds to
  `check_st_mint_entry`, plus the PoC test modules. Applies to upstream
  `jito-foundation/jito-tip-router` at `e77bcb4` or `1819052` (the touched
  files are identical between the two).
- `pocs/core/poc_weight_manipulation.rs` — core-level unit PoC. Computes the
  stake-weight scaling and attacker share under extreme `no_feed_weight`
  values. Tests core logic only; no program binary needed.
- `pocs/integration/poc_weight_manipulation_attack.rs` — integration tests
  demonstrating that extreme values are accepted by the program (pre-fix
  behavior). Requires the full integration-test fixture (restaking program
  `.so`).

## Honest caveats

- The patch is WIP and internally inconsistent: the PoC integration tests
  assert acceptance of extreme values, while the bounds added by the same
  patch reject them. Run the integration tests against the unpatched
  upstream tree if you want to observe the pre-fix behavior.
- The core unit PoC passes (`cargo test -p jito-tip-router-core --
  poc_weight_manipulation`). The integration tests were not run in this
  session.
- Code derives from `jito-foundation/jito-tip-router`; see the upstream
  repository for licensing.
