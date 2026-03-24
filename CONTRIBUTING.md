# Contributing to StellarForge

Thanks for your interest in contributing. This guide covers everything you need to get set up, run tests, meet code style requirements, and submit a pull request.

---

## Prerequisites & Local Setup

You will need the following tools installed before you can build or test the contracts.

### Rust

- **Edition:** 2021
- **Target:** `wasm32v1-none`

Install the required target:

```bash
rustup target add wasm32v1-none
```

If you don't have Rust installed, follow the official guide at <https://www.rust-lang.org/tools/install>.

### Stellar CLI

**v25.2.0 or higher** is required.

```bash
cargo install --locked stellar-cli
```

Full installation docs: <https://developers.stellar.org/docs/smart-contracts/getting-started/setup>

### Clone and Build

```bash
git clone https://github.com/your-org/stellarforge.git
cd stellarforge
cargo build --workspace
```

---

## Running Tests

Run the full test suite across all workspace members:

```bash
cargo test --workspace
```

Run tests for a single contract using the `-p` flag:

```bash
cargo test -p forge-governor
cargo test -p forge-multisig
cargo test -p forge-oracle
cargo test -p forge-stream
cargo test -p forge-vesting
```

All tests must pass before you submit a PR.

---

## Testing Philosophy

We believe comprehensive tests are essential for smart contract reliability. Good tests prevent bugs, document expected behavior, and give contributors confidence when making changes.

### What to Test

Every contract function should have tests covering:

1. **Happy paths** — Normal, expected usage with valid inputs
2. **Error paths** — Invalid inputs, unauthorized access, and edge cases that should fail gracefully
3. **Boundary conditions** — Limits, thresholds, and transition points (e.g., exactly at staleness boundary, one second past)
4. **State transitions** — Verify state changes persist correctly (e.g., price updates overwrite previous values)

### Test Structure and Naming

- Place tests in a `#[cfg(test)]` module at the bottom of `lib.rs`
- Name test functions descriptively: `test_<action>_<condition>_<expected_result>`
  - Good: `test_submit_price_with_zero_value_rejected`
  - Good: `test_get_price_at_exact_staleness_boundary_succeeds`
  - Avoid: `test_price`, `test_error_case`
- Use a `setup()` helper function to reduce boilerplate
- Group related tests with comments (e.g., `// ── Staleness boundary tests ──`)

### Using Soroban's Test Environment

Soroban provides powerful testing utilities. Key patterns:

```rust
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Env,
};

// Mock all authorization checks
env.mock_all_auths();

// Manipulate ledger time for staleness/expiry tests
env.ledger().with_mut(|l| l.timestamp = 1000);

// Generate test addresses
let admin = Address::generate(&env);

// Test error cases with try_ methods
let result = client.try_submit_price(&base, &quote, &0);
assert_eq!(result, Err(Ok(OracleError::InvalidPrice)));
```

### Example of a Well-Written Test

```rust
/// Verify that submitting a new price for an existing pair overwrites the old one.
/// This ensures stale prices are not retained.
#[test]
fn test_price_update_overwrites_previous_price() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = setup(&env);

    let base = Symbol::new(&env, "XLM");
    let quote = Symbol::new(&env, "USDC");

    // Submit initial price at timestamp 1000
    env.ledger().with_mut(|l| l.timestamp = 1000);
    let initial_price = 10_000_000i128;
    client.submit_price(&base, &quote, &initial_price);

    // Verify initial price is stored
    let data = client.get_price(&base, &quote);
    assert_eq!(data.price, initial_price);
    assert_eq!(data.updated_at, 1000);

    // Submit new price for the same pair at timestamp 2000
    env.ledger().with_mut(|l| l.timestamp = 2000);
    let new_price = 15_000_000i128;
    client.submit_price(&base, &quote, &new_price);

    // Verify get_price() returns the new price, not the old one
    let data = client.get_price(&base, &quote);
    assert_eq!(data.price, new_price, "Expected new price to overwrite old price");
    assert_eq!(data.updated_at, 2000, "Expected timestamp to be updated");
}
```

This test demonstrates:
- Clear documentation explaining what's being tested
- Descriptive variable names and comments
- Testing both the action and its side effects
- Explicit assertions with helpful failure messages
- Time manipulation to test state changes

### When Adding New Tests

- If you're fixing a bug, add a test that would have caught it
- If you're adding a feature, test both success and failure cases
- If you're modifying existing behavior, update related tests
- Run `cargo test -p <contract-name>` frequently during development

---

## Code Style

### Formatting

```bash
cargo fmt --all
```

This must produce no changes. Run it before committing.

### Linting

```bash
cargo clippy --all-targets -- -D warnings
```

This must produce zero warnings.

### Additional Rules

- New public functions and types require `///` doc comments.
- No `unsafe` code is permitted in any contract.
- No external crate dependencies beyond `soroban-sdk` are permitted without prior discussion with maintainers.

---

## Pull Request Process

1. Fork the repository and create a feature branch off `main`.
2. Make your changes, keeping commits logically atomic (or squash before opening the PR).
3. Ensure all CI checks pass locally before requesting review:
   - `cargo fmt --all` — no changes
   - `cargo clippy --all-targets -- -D warnings` — zero warnings
   - `cargo test --workspace` — all tests pass
4. Open a PR against `main`. Your PR description must summarise what changed and why.
5. If your PR introduces a new contract or public API, include tests covering error paths and state transitions.
6. At least one maintainer approval is required before a PR is merged.

---

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
