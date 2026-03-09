# ⚒️ StellarForge

**Reusable Soroban smart contract primitives for the Stellar ecosystem.**

StellarForge is a collection of production-ready, well-tested Soroban contracts that developers can deploy directly or use as building blocks for more complex DeFi applications on Stellar.

---

## Contracts

| Contract | Description |
|---|---|
| [`forge-vesting`](./contracts/forge-vesting) | Token vesting with cliff + linear release |
| [`forge-stream`](./contracts/forge-stream) | Real-time per-second token streaming |
| [`forge-multisig`](./contracts/forge-multisig) | N-of-M treasury with timelock |
| [`forge-governor`](./contracts/forge-governor) | Token-weighted on-chain governance |
| [`forge-oracle`](./contracts/forge-oracle) | Price feed with staleness protection |

---

## forge-vesting

Deploy tokens on a vesting schedule with an optional cliff period.

```
initialize(token, beneficiary, admin, total_amount, cliff_seconds, duration_seconds)
claim()           → withdraws all currently unlocked tokens
cancel()          → admin cancels, returns unvested tokens
get_status()      → VestingStatus { total, claimed, vested, claimable, cliff_reached, fully_vested }
```

**Example:** 1M tokens, 6-month cliff, 2-year linear vesting.

---

## forge-stream

Pay-per-second token streams. Recipients withdraw accrued tokens at any time.

```
create_stream(sender, token, recipient, rate_per_second, duration_seconds) → stream_id
withdraw(stream_id)        → recipient pulls accrued tokens
cancel_stream(stream_id)   → sender cancels, splits remaining tokens fairly
get_stream_status(id)      → StreamStatus { streamed, withdrawn, withdrawable, remaining, is_active }
```

**Example:** Stream 100 USDC/day to a contractor for 30 days.

---

## forge-multisig

N-of-M treasury requiring multiple owner approvals before funds move.

```
initialize(owners, threshold, timelock_delay)
propose(proposer, to, token, amount)  → proposal_id
approve(owner, proposal_id)
reject(owner, proposal_id)
execute(executor, proposal_id)        → transfers funds after timelock
```

**Example:** 3-of-5 multisig with 24-hour timelock for a DAO treasury.

---

## forge-governor

Token-weighted on-chain governance with configurable quorum and timelock.

```
initialize(GovernorConfig { vote_token, voting_period, quorum, timelock_delay })
propose(proposer, title, description)  → proposal_id
vote(voter, proposal_id, support, weight)
finalize(proposal_id)                  → sets state to Passed or Failed
execute(executor, proposal_id)         → marks proposal executed after timelock
```

**Example:** Protocol parameter changes voted on by token holders.

---

## forge-oracle

Admin-controlled price feeds with staleness protection.

```
initialize(admin, staleness_threshold)
submit_price(base, quote, price)       → admin submits price (7 decimal places)
get_price(base, quote)                 → PriceData, reverts if stale
get_price_unsafe(base, quote)          → PriceData, no staleness check
set_staleness_threshold(new_threshold)
transfer_admin(new_admin)
```

**Example:** XLM/USDC price feed updated every 60 seconds.

---

## Getting Started

### Prerequisites

- Rust + `wasm32-unknown-unknown` target
- Stellar CLI

```bash
rustup target add wasm32-unknown-unknown
cargo install stellar-cli
```

### Build all contracts

```bash
cargo build --workspace
stellar contract build
```

### Run all tests

```bash
cargo test --workspace
```

### Run a specific contract's tests

```bash
cargo test -p forge-vesting
cargo test -p forge-stream
cargo test -p forge-multisig
cargo test -p forge-governor
cargo test -p forge-oracle
```

---

## Design Principles

- **No unsafe code** — all contracts are `#![no_std]` and fully safe Rust
- **Minimal dependencies** — only `soroban-sdk`, no external crates
- **Comprehensive tests** — every error path and state transition is covered
- **Clear error types** — typed error enums with descriptive variants
- **Event emission** — all state changes emit events for off-chain indexing

---

## Contributing

PRs welcome. Please ensure:
- `cargo fmt --all` passes
- `cargo clippy --all-targets -- -D warnings` passes
- `cargo test --workspace` passes
- New functions have `///` doc comments

---

## License

MIT
