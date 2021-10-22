use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, serde_json, PromiseOrValue};

use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

use crate::sale::Sale;
use crate::*;

const GAS_GET_ACCOUNT_STAKED_BALANCE: Gas = Gas(25_000_000_000_000);
const GAS_ON_GET_ACCOUNT_STAKED_BALANCE: Gas = Gas(25_000_000_000_000);
const NO_DEPOSIT: Balance = 0;

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    /// Check the staked balance of the given account.
    fn get_account_staked_balance(&self, account_id: AccountId) -> U128;
}

#[ext_contract(ext_self)]
pub trait ExtContract {
    /// Callback from checking staked balance of the given user.
    fn on_get_account_staked_balance(
        &mut self,
        sale_id: u64,
        token_id: AccountId,
        sender_id: AccountId,
        deposit_amount: U128,
    ) -> PromiseOrValue<U128>;

    /// Callback after account creation.
    fn on_create_account(&mut self, new_account_id: AccountId) -> Promise;
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleDeposit {
    pub sale_id: u64,
    /// Optional argument to point to the contract where this user has staked if sale requires this.
    pub staking_contract: Option<AccountId>,
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// Callback on receiving tokens by this contract.
    /// Record the AccountSale for given Sale.
    #[allow(unused_variables)]
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        // Check that account is registered.
        let _ = self
            .accounts
            .get(&sender_id)
            .expect("ERR_NOT_REGISTERED_ACCOUNT");
        let message = serde_json::from_str::<SaleDeposit>(&msg).expect("ERR_MSG_WRONG_FORMAT");
        let sale: Sale = self
            .sales
            .get(&message.sale_id)
            .expect("ERR_NO_SALE")
            .into();
        assert_eq!(
            sale.deposit_token_id,
            env::predecessor_account_id(),
            "ERR_WRONG_TOKEN"
        );
        if sale.hard_max_amount_limit {
            assert!(
                sale.collected_amount < sale.max_amount.expect("ERR_NO_MAX_AMOUNT"),
                "ERR_SALE_DONE"
            );
        }
        let timestamp = env::block_timestamp();
        assert!(timestamp >= sale.start_date, "ERR_SALE_NOT_STARTED");
        assert!(
            timestamp >= sale.start_date && timestamp <= sale.end_date,
            "ERR_SALE_DONE"
        );

        // Send call to check how much is staked if staking is required.
        if sale.staking_contracts.len() > 0 {
            let staking_contract = message
                .staking_contract
                .expect("ERR_MUST_HAVE_STAKING_CONTRACT");
            assert!(
                sale.staking_contracts.contains(&staking_contract),
                "ERR_NOT_WHITELISTED_STAKING_CONTRACT"
            );
            PromiseOrValue::Promise(
                ext_staking_pool::get_account_staked_balance(
                    sender_id.clone(),
                    staking_contract,
                    NO_DEPOSIT,
                    GAS_GET_ACCOUNT_STAKED_BALANCE,
                )
                .then(ext_self::on_get_account_staked_balance(
                    message.sale_id,
                    env::predecessor_account_id(),
                    sender_id,
                    amount,
                    env::current_account_id(),
                    NO_DEPOSIT,
                    GAS_ON_GET_ACCOUNT_STAKED_BALANCE,
                )),
            )
        } else {
            PromiseOrValue::Value(U128(self.internal_sale_deposit(
                message.sale_id,
                &env::predecessor_account_id(),
                &sender_id,
                0,
                amount.0,
            )))
        }
    }
}
