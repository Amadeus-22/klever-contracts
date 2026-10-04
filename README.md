# klever-contracts

Smart contracts for [KleverChain](https://klever.org), written in Rust and compiled to WASM for the Klever VM (KVM).

**The problem.** The setup the official Klever tooling documents no longer works: the VS Code extension tells you to install Rust `nightly-2024-06-12`, and with it `ksc all build` fails on current dependencies (`feature edition2024 is required`). A newcomer loses an afternoon before compiling the first contract. This repository pins a toolchain that does build, and records the whole cycle — build, test, deploy, call — with commands that were run against testnet.

## Contracts

| Contract | What it does | Testnet |
|---|---|---|
| [`adder`](adder/) | The framework's starter template: keeps one number in storage, anyone can add to it. Used here to prove the toolchain end to end. | [`klv1qqqq…ry9n0t`](https://testnet.kleverscan.org/account/klv1qqqqqqqqqqqqqpgqmlw0q7jafgq6wc68snyhzq2zg6mhfxct3vqqry9n0t) |

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

### Recorded run (2026-10-03)

| Step | Result |
|---|---|
| Deploy, `init(5)` | tx `ef6ed077a8284571966ebd05c13eb52af0d2951710e26f54d430bc5b0344f657`, success |
| `getSum` | `5` |
| `add(7)` | tx `05d02b69d2d41001ca4cf96ff9bedf696b89735e840a0b81f5d330a9de7474c9`, success |
| `getSum` | `12` |

## Roadmap

1. `adder` on testnet — done.
2. A vault contract with a withdrawal limit, paired with a Go monitor that alerts when an account approaches it (the KleverChain counterpart of [chainwatch](https://github.com/Amadeus-22/chainwatch)).
