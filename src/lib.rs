use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, AccountId, Balance, BorshStorageKey, Gas, Promise, PublicKey};

use crate::sale::VSale;

mod sale;
mod token_receiver;

pub(crate) const ONE_YOCTO: Balance = 1;
pub(crate) const ONE_NEAR: Balance = 10u128.pow(24);

const BASE_GAS: Gas = Gas(5_000_000_000_000);
pub(crate) const CREATE_ACCOUNT_GAS: Gas = Gas(4 * BASE_GAS.0);

const REFERRAL_FEE_DENOMINATOR: u128 = 10000;
const NEAR_ACCOUNT: &str = "near";

#[derive(BorshSerialize, BorshDeserialize)]
struct Account {
    referrer: AccountId,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Accounts,
    Sales,
    AccountSales { sale_id: u64 },
}

#[near_bindgen]
struct Contract {
    owner_id: AccountId,
    join_fee: Balance,
    referral_fees: Vec<u64>,
    accounts: LookupMap<AccountId, Account>,
    sales: LookupMap<u64, VSale>,
    num_sales: u64,
}

#[near_bindgen]
impl Contract {
    pub fn new(owner_id: AccountId, join_fee: U128, referral_fees: Vec<u64>) -> Self {
        Self {
            owner_id,
            join_fee: join_fee.0,
            referral_fees,
            accounts: LookupMap::new(StorageKey::Accounts),
            sales: LookupMap::new(StorageKey::Sales),
            num_sales: 0,
        }
    }

    pub fn create_account(&mut self, account_id: AccountId, public_key: PublicKey) -> Promise {
        Promise::new(AccountId::new_unchecked(NEAR_ACCOUNT.to_string())).function_call(
            "create_account".to_string(),
            format!(
                "{{\"new_account_id\": \"{}\", \"new_public_key\": \"{:?}\"}}",
                account_id, public_key
            )
            .into_bytes(),
            env::storage_byte_cost() * 500,
            CREATE_ACCOUNT_GAS,
        )
    }

    #[payable]
    pub fn join(&mut self) {
        let account_id = env::predecessor_account_id();
        assert!(
            self.accounts.get(&account_id).is_none(),
            "ERR_ACCOUNT_EXISTS"
        );
        assert_eq!(env::attached_deposit(), self.join_fee, "ERR_FEE");
        self.accounts.insert(
            &account_id,
            &Account {
                referrer: self.owner_id.clone(),
            },
        );
    }

    pub fn get_join_fee(&self) -> U128 {
        U128(self.join_fee)
    }

    pub fn get_referral_fees(&self) -> Vec<u64> {
        self.referral_fees.clone()
    }

    pub fn get_account(&self, account_id: AccountId) -> Account {
        self.accounts
            .get(&account_id)
            .expect("ERR_ACCOUNT_DOESNT_EXIST")
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::test_utils::{accounts, testing_env_with_promise_results};
    use near_sdk::{serde_json, testing_env, PromiseResult};

    use crate::sale::SaleInput;

    use super::*;
    use crate::token_receiver::SaleDeposit;
    use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

    fn contract_with_sale() -> (VMContextBuilder, Contract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context.predecessor_account_id(accounts(0)).build());
        let mut contract = Contract::new(accounts(0), U128(1_000_000), vec![10, 20, 30]);
        contract.create_sale(SaleInput {
            staking_contract: Some(AccountId::new_unchecked("test.staking".to_string())),
            min_near_deposit: U128(100),
            deposit_token_id: accounts(1),
            min_buy: U128(100),
            max_buy: U128(10000),
            max_amount: U128(10000),
            price: U128(1000),
            whitelist_hash: None,
        });
        (context, contract)
    }

    #[test]
    fn test_basics() {
        let (mut context, mut contract) = contract_with_sale();
        assert_eq!(contract.get_num_sales(), 1);
        assert_eq!(contract.get_sale(0).price.0, 1000);
        assert_eq!(contract.get_sales(0, 10).len(), 1);

        testing_env!(context
            .predecessor_account_id(accounts(2))
            .attached_deposit(1000000)
            .build());
        contract.join();

        testing_env!(context.predecessor_account_id(accounts(1)).build());
        contract.ft_on_transfer(
            accounts(2),
            U128(100),
            serde_json::to_string(&SaleDeposit { sale_id: 0 }).unwrap(),
        );

        testing_env_with_promise_results(
            context
                .current_account_id(accounts(0))
                .predecessor_account_id(accounts(0))
                .build(),
            PromiseResult::Successful(vec![]),
        );
        contract.on_get_account_staked_balance(U128(1000), 0, accounts(1), accounts(2), U128(100));

        assert_eq!(contract.get_sale(0).num_account_sales, 1);
        assert_eq!(contract.get_sale(0).collected_amount.0, 100);
    }

    #[test]
    #[should_panic(expected = "ERR_NOT_REGISTERED_ACCOUNT")]
    fn test_not_registered() {
        let (mut context, mut contract) = contract_with_sale();
        testing_env!(context.predecessor_account_id(accounts(1)).build());
        contract.ft_on_transfer(
            accounts(2),
            U128(100),
            serde_json::to_string(&SaleDeposit { sale_id: 0 }).unwrap(),
        );
    }

    #[test]
    #[should_panic(expected = "ERR_NO_SALE")]
    fn test_no_sale() {
        let (mut context, mut contract) = contract_with_sale();
        testing_env!(context
            .predecessor_account_id(accounts(2))
            .attached_deposit(1000000)
            .build());
        contract.join();
        testing_env!(context.predecessor_account_id(accounts(1)).build());
        contract.ft_on_transfer(
            accounts(2),
            U128(100),
            serde_json::to_string(&SaleDeposit { sale_id: 1 }).unwrap(),
        );
    }
}
