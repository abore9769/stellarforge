# ⚒️ StellarForge

**Reusable Soroban smart contract primitives for the Stellar ecosystem.**

StellarForge is a collection of production-ready, well-tested Soroban contracts that developers can deploy directly or use as building blocks for more complex DeFi applications on Stellar.

---

## 📊 Contract Comparison
Developers evaluating StellarForge can use this table to quickly identify the right primitive for their specific use case.

| Contract | Use Case | Admin Required | Events Emitted | Timelock |
| :--- | :--- | :--- | :--- | :--- |
| [`forge-governor`](#forge-governor) | Governance | No (Auth-based) | None | Yes (Voting/Execution delay) |
| [`forge-multisig`](#forge-multisig) | Multisig Treasury | Yes (Owners) | None | Yes (Post-approval delay) |
| [`forge-oracle`](#forge-oracle) | Price Feed | Yes (Admin) | `price_updated` | No |
| [`forge-stream`](#forge-stream) | Real-time Payments | No (Stream-specific) | `stream_created`, `withdrawn`, `stream_cancelled` | No |
| [`forge-vesting`](#forge-vesting) | Token Vesting | Yes (Admin) | `vesting_initialized`, `claimed`, `vesting_cancelled` | Yes (Cliff period) |

---

## 📜 Contract Details

### forge-vesting
Deploy tokens on a vesting schedule with an optional cliff period. Perfect for team allocations or advisor tokens.

* **Key Function:** `initialize(token, beneficiary, admin, total_amount, cliff_seconds, duration_seconds)`
* **Action:** `claim()` withdraws all currently unlocked tokens.
* **Security:** `cancel()` allows the admin to return unvested tokens if a contributor leaves.

### forge-stream
Pay-per-second token streams. Ideal for payroll, subscriptions, or real-time contractor payments.

* **Key Function:** `create_stream(sender, token, recipient, rate_per_second, duration_seconds)`
* **Action:** `withdraw(stream_id)` allows the recipient to pull accrued tokens at any time.
* **Pause/Resume:** `pause_stream(stream_id)` and `resume_stream(stream_id)` allow senders to temporarily halt or restart token accrual.

### forge-multisig
An N-of-M treasury requiring multiple owner approvals before funds move. Essential for DAO treasuries.

* **Key Function:** `propose(proposer, to, token, amount)`
* **Action:** `execute(executor, proposal_id)` transfers funds only after the configured timelock.
* **Duplicate Owners:** If duplicate addresses are provided during initialization, they are automatically deduplicated to ensure each owner is unique and counts only once toward the threshold.

### forge-governor
Token-weighted on-chain governance with configurable quorum and voting periods.

* **Key Function:** `propose(proposer, title, description)`
* **Action:** Supports token-weighted voting and automated execution after a passed proposal.

### forge-oracle
Admin-controlled price feeds with staleness protection for DeFi protocols.

* **Key Function:** `submit_price(base, quote, price)`
* **Security:** `get_price(base, quote)` reverts if data is older than the staleness threshold.

---

## 🛠️ Prerequisites & Setup

Soroban is Stellar’s smart contract platform, built for performance and developer-friendly Rust tooling. Learn more in the [official docs](https://developers.stellar.org/docs/smart-contracts/overview).

To build and test these contracts, you will need the following tools:

#### Rust Requirements
- **Rust Edition:** 2021
- **Target:** `wasm32v1-none` (v1 instruction set recommended for Soroban)

```bash
rustup target add wasm32v1-none
```

#### CLI Installation
The `stellar-cli` is essential for building, deploying, and interacting with Soroban contracts. **v25.2.0 or higher** is recommended.

```bash
cargo install --locked stellar-cli
```

#### Funding Testnet Accounts
Before deploying, you'll need a funded testnet account. You can generate and fund one easily:

```bash
stellar keys generate <identity_name> --network testnet --fund
```

### Using Make (Recommended)
This project includes a Makefile with common development commands:

| Command | Description |
| :--- | :--- |
| `make build` | Build all workspace crates |
| `make test` | Run all tests |
| `make lint` | Run clippy linter with deny warnings |
| `make fmt` | Format code |
| `make check` | Run fmt + lint + test in sequence |
| `make clean` | Clean build artifacts |

### Build all contracts

```bash
make build
# or manually:
cargo build --workspace
stellar contract build
```

### Run all tests

```bash
make test
# or manually:
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

## 📐 State Diagrams

Visual lifecycle documentation for stateful contracts is available in [`docs/state-diagrams.md`](docs/state-diagrams.md).

| Contract | States |
| :--- | :--- |
| `forge-vesting` | Active → Cliff Reached → Fully Vested → Cancelled |
| `forge-stream` | Active → Finished / Cancelled |
| `forge-governor` | Active → Passed / Failed → Executed |

---

## 📖 Glossary

Understanding these key terms will help you work with StellarForge contracts more effectively:

**Cliff** — A waiting period before any tokens become available. In [vesting](#forge-vesting), no tokens can be claimed until the cliff period expires, even though time is accruing. Common for employee token grants (e.g., 1-year cliff).

**Vesting** — The gradual release of tokens over time according to a predefined schedule. After the cliff (if any), tokens unlock linearly until the full amount is available. See [`forge-vesting`](#forge-vesting).

**Stream** — Continuous, per-second token flow from sender to recipient. Unlike vesting, streams have no cliff and tokens accrue in real-time. Perfect for payroll or subscriptions. See [`forge-stream`](#forge-stream).

**Timelock** — A mandatory delay between approval and execution of an action. Used in [`forge-multisig`](#forge-multisig) (post-approval delay) and [`forge-governor`](#forge-governor) (voting + execution delays) to allow stakeholders time to react.

**Quorum** — The minimum amount of voting power (token weight) required for a governance proposal to be valid. In [`forge-governor`](#forge-governor), proposals fail if they don't meet quorum, even with majority support.

**Multisig** — Short for "multi-signature." A wallet or treasury that requires M-of-N owners to approve transactions before execution. See [`forge-multisig`](#forge-multisig).

**Threshold** — The minimum number of approvals required in a multisig setup. For example, a 3-of-5 multisig has a threshold of 3, meaning 3 out of 5 owners must approve.

**Price Feed** — A data source providing asset price information to smart contracts. [`forge-oracle`](#forge-oracle) allows admins to submit prices for DeFi protocols to consume.

**Staleness** — How outdated price data is. In [`forge-oracle`](#forge-oracle), the staleness threshold defines the maximum age of price data before it's considered invalid and queries revert.

**Staleness Threshold** — The maximum time (in seconds) that price data remains valid in [`forge-oracle`](#forge-oracle). After this period, the data is considered stale and cannot be used.

---

## Design Principles

- **No unsafe code** — all contracts are `#![no_std]` and fully safe Rust
- **Minimal dependencies** — only `soroban-sdk`, no external crates
- **Comprehensive tests** — every error path and state transition is covered
- **Clear error types** — typed error enums with descriptive variants
- **Event emission** — all state changes emit events for off-chain indexing

---

## 📦 Versioning

StellarForge contracts follow [Semantic Versioning](https://semver.org/) (SemVer) to help you manage upgrades safely.

### Version Format: MAJOR.MINOR.PATCH

- **MAJOR** — Breaking changes that require action from developers
- **MINOR** — New features that are backward-compatible
- **PATCH** — Bug fixes and internal improvements

### What Counts as a Breaking Change?

Breaking changes require a MAJOR version bump and include:

- **Interface Changes** — Modifying function signatures, parameter types, or return values
- **Storage Layout Changes** — Altering contract storage structure in ways that break existing deployments
- **Behavior Changes** — Changing core logic that affects expected outcomes (e.g., calculation methods, state transitions)
- **Error Changes** — Removing or renaming error types that external code may depend on
- **Event Changes** — Modifying event structures or removing events that indexers rely on

### Non-Breaking Changes

These are safe and result in MINOR or PATCH bumps:

- Adding new optional functions
- Adding new events (without modifying existing ones)
- Internal optimizations that don't affect external behavior
- Bug fixes that restore intended behavior
- Documentation improvements

### Upgrade Recommendations

- **Review the [CHANGELOG.md](CHANGELOG.md)** before upgrading to understand what changed
- **Test thoroughly** on testnet before deploying MAJOR version upgrades to production
- **Pin versions** in your deployment scripts to avoid unexpected changes
- **Subscribe to releases** on GitHub to stay informed about security patches

### Contract Independence

Each contract in StellarForge is versioned independently. A breaking change in `forge-vesting` does not affect `forge-stream` versions.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, code style requirements, and the pull request process.

## Community & Discussions

Have a question, idea, or something to share? Join the conversation in [GitHub Discussions](https://github.com/soma-enyi/stellarforge/discussions) — we have dedicated spaces for Q&A, ideas, show-and-tell, and general chat.

---

## License

MIT