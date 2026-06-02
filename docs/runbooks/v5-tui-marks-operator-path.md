# V5 TUI marks operator path

Purpose: give operators a short path for producing and burning V5 marks from the TUI during devnet or CY soak checks.

## Path

1. Start the node/devnet and open `pwm-tui` against the active RPC endpoint.
2. Select an owner wallet row with PWM balance.
3. Press `S` and stake at least 1 whole PWM.
4. Wait for chain head height to advance. Marks accrue lazily while PWM is staked; with the default `blocks_per_hour=3600`, one nominal hour of blocks is needed for one hour of mark generation.
5. Watch the Marks column and the selected-account detail pane. The table/detail may show effective marks before a state-changing touch stores them.
6. Press `F5` once burnable marks are available. Burn touches the owner, materializes lazy marks, then burns the requested amount.
7. Use `U`, transfer, or another burn as a touch path when the account needs materialization.

## Important distinction

`ClaimTx` / `claim_mark` is retired in V5 and is not a TUI marks path.

`ClaimIPv4Batch` is a separate IPv4 allocation transaction used by registry flows. It is not the F5 marks burn journey.

## Quick wording

V5 marks path: stake PWM with `S`, wait for block height to advance, watch Marks grow, then burn with `F5`.
