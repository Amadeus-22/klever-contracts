# klever-contracts

Smart contracts for [KleverChain](https://klever.org), written in Rust and compiled to WASM for the Klever VM (KVM).

**The problem.** The setup the official Klever tooling documents no longer works: the VS Code extension tells you to install Rust `nightly-2024-06-12`, and with it `ksc all build` fails on current dependencies (`feature edition2024 is required`). A newcomer loses an afternoon before compiling the first contract. This repository pins a toolchain that does build, and records the whole cycle — build, test, deploy, call — with commands that were run against testnet.

## Contracts

| Contract | What it does | Testnet |
|---|---|---|
| [`vault`](vault/) | A KLV vault with a spending limit: the owner funds it and names one spender, who can withdraw at most `limit` per period. A stolen spender key can take one period's limit, not the balance. | [`klv1qqqq…5gvgsc`](https://testnet.kleverscan.org/account/klv1qqqqqqqqqqqqqpgqprrwnlul05prr753xm278kq6jexa0a523vqq5gvgsc) |
| [`adder`](adder/) | The framework's starter template: keeps one number in storage, anyone can add to it. Used here to prove the toolchain end to end. | [`klv1qqqq…ry9n0t`](https://testnet.kleverscan.org/account/klv1qqqqqqqqqqqqqpgqmlw0q7jafgq6wc68snyhzq2zg6mhfxct3vqqry9n0t) |

## vault

| Endpoint | Who | Does |
|---|---|---|
| `init(spender, limit, period_seconds)` | deployer | Sets the spender, the limit per period and the period length. |
| `deposit` (payable KLV) | anyone | Adds KLV to the vault. |
| `withdraw(amount)` | spender | Sends `amount` to the spender if the period's total stays within the limit. |
| `ownerWithdraw(amount)` | owner | Sends `amount` to the owner, with no limit. |
| `setLimit(limit)` | owner | Changes the limit; what was already spent this period stays counted. |
| `setSpender(address)` | owner | Replaces the spender, for example after a key is lost or stolen. |
| `getLimit`, `getSpent`, `getRemaining`, `getSpender`, `getPeriodSeconds` | views | Current state. `getSpent` and `getRemaining` already reflect a period that has rolled over. |

Periods are fixed windows counted from deployment, and unused allowance does not
carry over. 13 tests in [`vault/tests`](vault/tests/vault_blackbox_test.rs) cover
the limit, the period boundary, access control and rejected inputs.

[permwatch](https://github.com/Amadeus-22/permwatch) reads these views and warns
when the spending approaches the limit: `permwatch vault <contract>`.

### Recorded run on testnet (2026-10-03)

Limit 10 KLV per hour (`10000000` units; KLV has 6 decimals), spender
`klv1x4lh…e787`.

| Step | Result |
|---|---|
| Deploy | tx `c59ba8e7f016d394e236f2e5e97e98fe241332fa7f9868c78b78493b0a93ed39` |
| Owner deposits 50 KLV | tx `ae20ad30356f7e00c5316474bc3ff7a93f4516b3162f855ce269f33650643db4`; vault balance `50000000` |
| Spender withdraws 4 KLV | tx `4c1c6e9f834872a93ff5f557513668fe36092e19ae550ec275002bbb168ffb4d`; `getSpent` `4000000`, `getRemaining` `6000000` |
| Spender tries 7 KLV more | rejected: `withdrawal exceeds the limit of this period`; state unchanged |
| Spender withdraws 6 KLV | tx `f22ab15ff774f0edb7bbd934c852fa115cdb7b360ac6bfe764367673c7d02a6f`; `getRemaining` `0` |

## Toolchain

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.89.0 (stable) | Pinned in `rust-toolchain.toml`; the same channel the [klever-vm-sdk-rs](https://github.com/klever-io/klever-vm-sdk-rs) repository uses. |
| WASM target | `wasm32v1-none` | Installed by `rustup` from `rust-toolchain.toml`. `wasm32-unknown-unknown` is not enough. |
| `klever-sc` | 0.45.1 | The contract framework (crates.io). |
| `ksc` | 0.45.0 | Build tool, from the Klever SDK (`~/klever-sdk/ksc`). |
| `koperator` | from the Klever SDK | Signs and sends transactions (`~/klever-sdk/koperator`). |

The Klever SDK is downloaded by the "Klever Blockchain IDE" VS Code extension (`kleverchain: Setup Workspace`).

## Build and test

```bash
cd adder
~/klever-sdk/ksc all build     # writes output/adder.wasm (1411 bytes)
cargo test                     # 7 tests: unit, whitebox, blackbox and scenario
```

The same two commands work in `vault/`. After changing a contract's endpoints,
regenerate its typed proxy with `~/klever-sdk/ksc all proxy`.

## Deploy and call on testnet

The wallet is a testnet-only key kept **outside** this repository. `*.pem` is ignored by git; never commit one.

```bash
K=~/klever-sdk/koperator
W=~/.config/klever/testnet-wallet.pem
N=https://node.testnet.klever.org

$K account create -k $W                      # once
ADDR=$($K account address -k $W | grep -o 'klv1[a-z0-9]*')
curl -X POST "https://api.testnet.klever.org/v1.0/transaction/send-user-funds/$ADDR"   # faucet, once a day

# deploy with initial sum 5
$K sc create --wasm output/adder.wasm --upgradeable --readable --args bi:5 -k $W -n $N --await

# call add(7), then read the sum
C=<contract address>
$K sc invoke $C add --args bi:7 -k $W -n $N --await
curl -X POST "$N/vm/int" -H 'Content-Type: application/json' \
  -d "{\"scAddress\":\"$C\",\"funcName\":\"getSum\",\"args\":[]}"
```

The contract address is not printed by `sc create`; read it from
`https://api.testnet.klever.org/v1.0/sc/list?owner=<your address>`.

Two things that cost time here:

- **`no contract permission`** when sending a transaction: the account has a user
  permission at ID 0, so its owner permission moved to another ID. Pass the owner's
  ID, for example `--permID 1`. `permwatch audit <address>` lists the IDs.
- **Address and number arguments** are written `--args address:klv1…`,
  `--args bi:10000000` and `--args u64:3600`.

### Recorded run (2026-10-03)

| Step | Result |
|---|---|
| Deploy, `init(5)` | tx `ef6ed077a8284571966ebd05c13eb52af0d2951710e26f54d430bc5b0344f657`, success |
| `getSum` | `5` |
| `add(7)` | tx `05d02b69d2d41001ca4cf96ff9bedf696b89735e840a0b81f5d330a9de7474c9`, success |
| `getSum` | `12` |

## Roadmap

1. `adder` on testnet — done.
2. `vault` with a withdrawal limit, monitored by permwatch — done.
3. Vault for KDA tokens, not only KLV, with one limit per token.
