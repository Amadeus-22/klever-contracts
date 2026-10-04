#![no_std]

use klever_sc::imports::*;

pub mod vault_proxy;

/// A KLV vault with a spending limit.
///
/// The owner funds the vault and names one spender. The spender can withdraw at
/// most `limit` per period; the allowance does not carry over to the next period.
/// Only the owner can change the limit or the spender, or withdraw without a limit.
///
/// Periods are fixed windows of `period_seconds`, counted from deployment. A key
/// that is stolen from the spender can therefore take at most one period's limit
/// before the owner replaces it.
#[klever_sc::contract]
pub trait Vault {
    #[init]
    fn init(&self, spender: ManagedAddress, limit: BigUint, period_seconds: u64) {
        require!(
            period_seconds > 0,
            "period must be longer than zero seconds"
        );
        self.spender().set(spender);
        self.limit().set(limit);
        self.period_seconds().set(period_seconds);
        self.period_start()
            .set(self.blockchain().get_block_timestamp());
    }

    #[upgrade]
    fn upgrade(&self) {}

    /// Adds KLV to the vault. Anyone may deposit.
    #[endpoint]
    #[payable("KLV")]
    fn deposit(&self) {
        let amount = self.call_value().klv_value().clone_value();
        require!(amount > 0u64, "deposit must be more than zero");
        self.deposit_event(&self.blockchain().get_caller(), &amount);
    }

    /// Sends `amount` KLV to the spender, within the limit of the current period.
    #[endpoint]
    fn withdraw(&self, amount: BigUint) {
        let caller = self.blockchain().get_caller();
        require!(
            caller == self.spender().get(),
            "only the spender can withdraw"
        );
        require!(amount > 0u64, "amount must be more than zero");

        let spent_before = self.roll_period();
        let spent = spent_before + &amount;
        require!(
            spent <= self.limit().get(),
            "withdrawal exceeds the limit of this period"
        );

        self.spent().set(&spent);
        self.send().direct_klv(&caller, &amount);
        self.withdraw_event(&caller, &amount, &spent);
    }

    /// Sends `amount` KLV to the owner. Not subject to the limit.
    #[only_owner]
    #[endpoint(ownerWithdraw)]
    fn owner_withdraw(&self, amount: BigUint) {
        require!(amount > 0u64, "amount must be more than zero");
        let owner = self.blockchain().get_caller();
        self.send().direct_klv(&owner, &amount);
    }

    /// Changes the limit. The amount already spent in the current period stays counted.
    #[only_owner]
    #[endpoint(setLimit)]
    fn set_limit(&self, limit: BigUint) {
        self.limit().set(&limit);
        self.limit_changed_event(&limit);
    }

    /// Replaces the spender, for example after its key is lost or stolen.
    #[only_owner]
    #[endpoint(setSpender)]
    fn set_spender(&self, spender: ManagedAddress) {
        self.spender().set(&spender);
        self.spender_changed_event(&spender);
    }

    /// Amount withdrawn by the spender in the current period.
    #[view(getSpent)]
    fn spent_in_current_period(&self) -> BigUint {
        if self.current_period_start() == self.period_start().get() {
            self.spent().get()
        } else {
            BigUint::zero()
        }
    }

    /// Amount the spender can still withdraw in the current period.
    #[view(getRemaining)]
    fn remaining(&self) -> BigUint {
        let limit = self.limit().get();
        let spent = self.spent_in_current_period();
        if spent >= limit {
            BigUint::zero()
        } else {
            limit - spent
        }
    }

    /// Start of the period that contains the current block.
    fn current_period_start(&self) -> u64 {
        let start = self.period_start().get();
        let period = self.period_seconds().get();
        let now = self.blockchain().get_block_timestamp();
        if now < start {
            return start;
        }
        start + ((now - start) / period) * period
    }

    /// Moves to the current period if the stored one has ended, and returns the
    /// amount already spent in the current period.
    fn roll_period(&self) -> BigUint {
        let current = self.current_period_start();
        if current != self.period_start().get() {
            self.period_start().set(current);
            self.spent().clear();
        }
        self.spent().get()
    }

    #[view(getSpender)]
    #[storage_mapper("spender")]
    fn spender(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getLimit)]
    #[storage_mapper("limit")]
    fn limit(&self) -> SingleValueMapper<BigUint>;

    #[view(getPeriodSeconds)]
    #[storage_mapper("periodSeconds")]
    fn period_seconds(&self) -> SingleValueMapper<u64>;

    #[storage_mapper("periodStart")]
    fn period_start(&self) -> SingleValueMapper<u64>;

    #[storage_mapper("spent")]
    fn spent(&self) -> SingleValueMapper<BigUint>;

    #[event("deposit")]
    fn deposit_event(&self, #[indexed] from: &ManagedAddress, amount: &BigUint);

    #[event("withdraw")]
    fn withdraw_event(
        &self,
        #[indexed] spender: &ManagedAddress,
        #[indexed] amount: &BigUint,
        spent_in_period: &BigUint,
    );

    #[event("limitChanged")]
    fn limit_changed_event(&self, limit: &BigUint);

    #[event("spenderChanged")]
    fn spender_changed_event(&self, #[indexed] spender: &ManagedAddress);
}
