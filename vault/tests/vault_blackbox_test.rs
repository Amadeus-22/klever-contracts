use klever_sc::types::{Address, BigUint, TestAddress, TestSCAddress};
use klever_sc_scenario::imports::KleverscPath;
use klever_sc_scenario::*;
use vault::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SPENDER: TestAddress = TestAddress::new("spender");
const STRANGER: TestAddress = TestAddress::new("stranger");
const VAULT: TestSCAddress = TestSCAddress::new("vault");
const CODE_PATH: KleverscPath = KleverscPath::new("output/vault.kleversc.json");

const LIMIT: u64 = 100;
const PERIOD: u64 = 3_600;
const DEPLOYED_AT: u64 = 1_000_000;
/// Status the Klever VM returns when a contract rejects a call with `require!`.
const USER_ERROR: u64 = 57;

fn address(account: TestAddress) -> Address {
    Address::from(account.eval_to_array())
}

/// Deploys a vault holding 1000 KLV units: the spender may take 100 per hour.
fn deployed() -> ScenarioWorld {
    let mut world = ScenarioWorld::new();
    world.register_contract(CODE_PATH, vault::ContractBuilder);

    world.account(OWNER).nonce(1).balance(10_000);
    world.account(SPENDER).nonce(1).balance(0);
    world.account(STRANGER).nonce(1).balance(500);
    world.current_block().block_timestamp(DEPLOYED_AT);

    world
        .tx()
        .from(OWNER)
        .typed(vault_proxy::VaultProxy)
        .init(address(SPENDER), LIMIT, PERIOD)
        .code(CODE_PATH)
        .new_address(VAULT)
        .run();

    world
        .tx()
        .from(OWNER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .deposit()
        .klv(1_000u64)
        .run();

    world
}

fn withdraw(world: &mut ScenarioWorld, amount: u64) {
    world
        .tx()
        .from(SPENDER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .withdraw(amount)
        .run();
}

fn withdraw_fails(world: &mut ScenarioWorld, from: TestAddress, amount: u64, message: &str) {
    world
        .tx()
        .from(from)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .withdraw(amount)
        .returns(ExpectError(USER_ERROR, message))
        .run();
}

fn expect_spent_and_remaining(world: &mut ScenarioWorld, spent: u64, remaining: u64) {
    world
        .query()
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .spent_in_current_period()
        .returns(ExpectValue(BigUint::from(spent)))
        .run();
    world
        .query()
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .remaining()
        .returns(ExpectValue(BigUint::from(remaining)))
        .run();
}

#[test]
fn deploy_and_deposit() {
    let mut world = deployed();
    world.check_account(VAULT).balance(1_000);
    world.check_account(OWNER).balance(9_000);
    expect_spent_and_remaining(&mut world, 0, LIMIT);
}

#[test]
fn spender_withdraws_up_to_the_limit() {
    let mut world = deployed();

    withdraw(&mut world, 60);
    world.check_account(SPENDER).balance(60);
    world.check_account(VAULT).balance(940);
    expect_spent_and_remaining(&mut world, 60, 40);

    withdraw(&mut world, 40);
    world.check_account(SPENDER).balance(100);
    expect_spent_and_remaining(&mut world, 100, 0);
}

#[test]
fn withdrawal_over_the_limit_is_rejected_and_changes_nothing() {
    let mut world = deployed();
    withdraw(&mut world, 60);

    withdraw_fails(
        &mut world,
        SPENDER,
        41,
        "withdrawal exceeds the limit of this period",
    );
    world.check_account(SPENDER).balance(60);
    world.check_account(VAULT).balance(940);
    expect_spent_and_remaining(&mut world, 60, 40);
}

#[test]
fn only_the_spender_can_withdraw() {
    let mut world = deployed();
    withdraw_fails(&mut world, STRANGER, 1, "only the spender can withdraw");
    withdraw_fails(&mut world, OWNER, 1, "only the spender can withdraw");
    world.check_account(VAULT).balance(1_000);
}

#[test]
fn zero_withdrawal_is_rejected() {
    let mut world = deployed();
    withdraw_fails(&mut world, SPENDER, 0, "amount must be more than zero");
}

#[test]
fn allowance_resets_in_the_next_period_and_does_not_carry_over() {
    let mut world = deployed();
    withdraw(&mut world, 30);

    // Last second of the first period: still the same allowance.
    world
        .current_block()
        .block_timestamp(DEPLOYED_AT + PERIOD - 1);
    expect_spent_and_remaining(&mut world, 30, 70);

    // First second of the second period: the views already show a fresh allowance.
    world.current_block().block_timestamp(DEPLOYED_AT + PERIOD);
    expect_spent_and_remaining(&mut world, 0, LIMIT);

    // The 70 left unused in the first period is gone: only 100 is available.
    withdraw_fails(
        &mut world,
        SPENDER,
        101,
        "withdrawal exceeds the limit of this period",
    );
    withdraw(&mut world, 100);
    world.check_account(SPENDER).balance(130);
    expect_spent_and_remaining(&mut world, 100, 0);
}

#[test]
fn periods_are_fixed_windows_from_deployment() {
    let mut world = deployed();

    // Two and a half periods later, with no activity in between.
    world
        .current_block()
        .block_timestamp(DEPLOYED_AT + 2 * PERIOD + PERIOD / 2);
    withdraw(&mut world, 100);

    // The third period ends at DEPLOYED_AT + 3 * PERIOD, not one full period
    // after the withdrawal.
    world
        .current_block()
        .block_timestamp(DEPLOYED_AT + 3 * PERIOD - 1);
    expect_spent_and_remaining(&mut world, 100, 0);
    world
        .current_block()
        .block_timestamp(DEPLOYED_AT + 3 * PERIOD);
    expect_spent_and_remaining(&mut world, 0, LIMIT);
}

#[test]
fn owner_changes_the_limit_and_spent_stays_counted() {
    let mut world = deployed();
    withdraw(&mut world, 60);

    world
        .tx()
        .from(OWNER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .set_limit(50u64)
        .run();
    expect_spent_and_remaining(&mut world, 60, 0);
    withdraw_fails(
        &mut world,
        SPENDER,
        1,
        "withdrawal exceeds the limit of this period",
    );

    world
        .tx()
        .from(OWNER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .set_limit(200u64)
        .run();
    expect_spent_and_remaining(&mut world, 60, 140);
}

#[test]
fn owner_replaces_the_spender() {
    let mut world = deployed();

    world
        .tx()
        .from(OWNER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .set_spender(address(STRANGER))
        .run();

    withdraw_fails(&mut world, SPENDER, 1, "only the spender can withdraw");
    world
        .tx()
        .from(STRANGER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .withdraw(10u64)
        .run();
    world.check_account(STRANGER).balance(510);
}

#[test]
fn only_the_owner_can_administer() {
    let mut world = deployed();

    world
        .tx()
        .from(SPENDER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .set_limit(1_000_000u64)
        .returns(ExpectError(
            USER_ERROR,
            "Endpoint can only be called by owner",
        ))
        .run();
    world
        .tx()
        .from(SPENDER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .set_spender(address(STRANGER))
        .returns(ExpectError(
            USER_ERROR,
            "Endpoint can only be called by owner",
        ))
        .run();
    world
        .tx()
        .from(SPENDER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .owner_withdraw(1u64)
        .returns(ExpectError(
            USER_ERROR,
            "Endpoint can only be called by owner",
        ))
        .run();
    expect_spent_and_remaining(&mut world, 0, LIMIT);
}

#[test]
fn owner_withdraws_without_a_limit() {
    let mut world = deployed();

    world
        .tx()
        .from(OWNER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .owner_withdraw(1_000u64)
        .run();
    world.check_account(VAULT).balance(0);
    world.check_account(OWNER).balance(10_000);
    expect_spent_and_remaining(&mut world, 0, LIMIT);
}

#[test]
fn zero_deposit_is_rejected() {
    let mut world = deployed();
    world
        .tx()
        .from(STRANGER)
        .to(VAULT)
        .typed(vault_proxy::VaultProxy)
        .deposit()
        .returns(ExpectError(USER_ERROR, "deposit must be more than zero"))
        .run();
}

#[test]
fn zero_length_period_is_rejected_at_deploy() {
    let mut world = ScenarioWorld::new();
    world.register_contract(CODE_PATH, vault::ContractBuilder);
    world.account(OWNER).nonce(1);

    world
        .tx()
        .from(OWNER)
        .typed(vault_proxy::VaultProxy)
        .init(address(SPENDER), LIMIT, 0u64)
        .code(CODE_PATH)
        .new_address(VAULT)
        .returns(ExpectError(
            USER_ERROR,
            "period must be longer than zero seconds",
        ))
        .run();
}
