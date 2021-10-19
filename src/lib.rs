use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, near_bindgen, AccountId, Balance, BorshStorageKey, Gas, PanicOnDefault, Promise, PublicKey,
};

use crate::sale::VSale;
use crate::token_receiver::ext_self;

mod sale;
mod token_receiver;

pub(crate) const ONE_NEAR: Balance = 10u128.pow(24);

const BASE_GAS: Gas = Gas(5_000_000_000_000);
pub(crate) const CREATE_ACCOUNT_GAS: Gas = Gas(4 * BASE_GAS.0);
pub(crate) const ON_CREATE_ACCOUNT_GAS: Gas = Gas(4 * BASE_GAS.0);

const NO_DEPOSIT: Balance = 0;
const ACCESS_KEY_ALLOWANCE: Balance = ONE_NEAR / 100;
const CREATE_LINK_AMOUNT: Balance = ONE_NEAR / 100;
const CREATE_ACCOUNT_AMOUNT: Balance = ONE_NEAR / 100;

const REFERRAL_FEE_DENOMINATOR: u128 = 10000;
const NEAR_ACCOUNT: &str = "near";

#[derive(BorshSerialize, BorshDeserialize)]
struct Account {
    referrer: AccountId,
    links: UnorderedSet<PublicKey>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct AccountOutput {
    referrer: AccountId,
}

impl From<Account> for AccountOutput {
    fn from(account: Account) -> Self {
        Self {
            referrer: account.referrer,
        }
    }
}

impl Account {
    pub fn new(account_id: &AccountId, referrer: &AccountId) -> Self {
        Self {
            referrer: referrer.clone(),
            links: UnorderedSet::new(StorageKey::AccountLinks {
                account_id: account_id.clone(),
            }),
        }
    }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Accounts,
    Sales,
    AccountSales { sale_id: u64 },
    Links,
    AccountLinks { account_id: AccountId },
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
struct Contract {
    owner_id: AccountId,
    join_fee: Balance,
    referral_fees: Vec<u64>,
    accounts: UnorderedMap<AccountId, Account>,
    sales: LookupMap<u64, VSale>,
    links: LookupMap<PublicKey, AccountId>,
    num_sales: u64,
}

impl Contract {
    fn internal_remove_link(&mut self, account_id: AccountId, public_key: PublicKey) -> Promise {
        let mut account = self.accounts.get(&account_id).expect("ERR_NO_ACCOUNT");
        self.links.remove(&public_key);
        account.links.remove(&public_key);
        self.accounts.insert(&account_id, &account);
        Promise::new(env::current_account_id()).delete_key(public_key)
    }
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId, join_fee: U128, referral_fees: Vec<u64>) -> Self {
        let mut this = Self {
            owner_id,
            join_fee: join_fee.0,
            referral_fees,
            accounts: UnorderedMap::new(StorageKey::Accounts),
            sales: LookupMap::new(StorageKey::Sales),
            links: LookupMap::new(StorageKey::Links),
            num_sales: 0,
        };
        this.accounts.insert(
            &this.owner_id,
            &Account::new(&this.owner_id, &this.owner_id),
        );
        this
    }

    #[payable]
    pub fn create_link(&mut self, public_key: PublicKey) -> Promise {
        assert_eq!(env::attached_deposit(), CREATE_LINK_AMOUNT);
        let mut account = self
            .accounts
            .get(&env::predecessor_account_id())
            .expect("ERR_NO_ACCOUNT");
        assert!(self.links.get(&public_key).is_none(), "ERR_DUPLICATE_KEY");
        self.links
            .insert(&public_key, &env::predecessor_account_id());
        account.links.insert(&public_key);
        self.accounts
            .insert(&env::predecessor_account_id(), &account);
        Promise::new(env::current_account_id()).add_access_key(
            public_key,
            ACCESS_KEY_ALLOWANCE,
            env::current_account_id(),
            "create_account".to_string(),
        )
    }

    pub fn remove_link(&mut self, public_key: PublicKey) -> Promise {
        let account_id = self.links.get(&public_key).expect("ERR_NO_LINK");
        assert_eq!(
            account_id,
            env::predecessor_account_id(),
            "ERR_NOT_LINK_OWNER"
        );
        self.internal_remove_link(env::predecessor_account_id(), public_key)
    }

    /// Only can be called by the access key on this contract.
    /// Can be added via create_link.
    pub fn create_account(&mut self, account_id: AccountId, public_key: PublicKey) -> Promise {
        assert_eq!(env::predecessor_account_id(), env::current_account_id());
        Promise::new(AccountId::new_unchecked(NEAR_ACCOUNT.to_string()))
            .function_call(
                "create_account".to_string(),
                format!(
                    "{{\"new_account_id\": \"{}\", \"new_public_key\": \"{:?}\"}}",
                    account_id, public_key
                )
                .into_bytes(),
                CREATE_ACCOUNT_AMOUNT,
                CREATE_ACCOUNT_GAS,
            )
            .then(ext_self::on_create_account(
                account_id,
                env::current_account_id(),
                NO_DEPOSIT,
                ON_CREATE_ACCOUNT_GAS,
            ))
    }

    /// Callback after account was created by near.
    /// Add an internal account with referrer and remove the link info.
    pub fn on_create_account(&mut self, new_account_id: AccountId) -> Promise {
        assert_eq!(env::predecessor_account_id(), env::current_account_id());
        let referrer = self
            .links
            .get(&env::signer_account_pk())
            .expect("ERR_NO_LINK");
        self.accounts
            .insert(&new_account_id, &Account::new(&new_account_id, &referrer));
        self.internal_remove_link(env::predecessor_account_id(), env::signer_account_pk())
    }

    #[payable]
    pub fn join(&mut self) {
        let account_id = env::predecessor_account_id();
        assert!(
            self.accounts.get(&account_id).is_none(),
            "ERR_ACCOUNT_EXISTS"
        );
        assert_eq!(env::attached_deposit(), self.join_fee, "ERR_FEE");
        self.accounts
            .insert(&account_id, &Account::new(&account_id, &self.owner_id));
    }

    pub fn get_join_fee(&self) -> U128 {
        U128(self.join_fee)
    }

    pub fn get_referral_fees(&self) -> Vec<u64> {
        self.referral_fees.clone()
    }

    pub fn get_account(&self, account_id: AccountId) -> AccountOutput {
        self.accounts
            .get(&account_id)
            .expect("ERR_ACCOUNT_DOESNT_EXIST")
            .into()
    }

    pub fn get_num_accounts(&self) -> u64 {
        self.accounts.len()
    }

    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<(AccountId, AccountOutput)> {
        let keys = self.accounts.keys_as_vector();
        let values = self.accounts.values_as_vector();
        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| (keys.get(index).unwrap(), values.get(index).unwrap().into()))
            .collect()
    }

    pub fn get_link_referrer(&self, public_key: PublicKey) -> AccountId {
        self.links.get(&public_key).expect("ERR_NO_KEY")
    }

    pub fn get_links(&self, account_id: AccountId) -> Vec<PublicKey> {
        let account = self.accounts.get(&account_id).expect("ERR_NO_ACCOUNT");
        account.links.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::test_utils::{accounts, testing_env_with_promise_results};
    use near_sdk::{serde_json, testing_env, PromiseResult};

    use crate::sale::{SaleInput, SaleMetadata};
    use crate::token_receiver::SaleDeposit;

    use super::*;
    use near_sdk::json_types::U64;
    use std::str::FromStr;

    fn contract_with_sale_info(
        max_amount: Option<Balance>,
        start_date: u64,
        end_date: u64,
    ) -> (VMContextBuilder, Contract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context.predecessor_account_id(accounts(0)).build());
        let join_fee = U128(1_000_000);
        let referral_fees = vec![10, 20, 30];
        let mut contract = Contract::new(accounts(0), join_fee, referral_fees.clone());
        contract.create_sale(SaleInput {
            metadata: SaleMetadata {
                name: "test".to_string(),
                symbol: "TEST".to_string(),
                description: "".to_string(),
                smart_contract_url: "".to_string(),
                logo_url: "".to_string(),
            },
            staking_contract: Some(AccountId::new_unchecked("test.staking".to_string())),
            min_near_deposit: U128(100),
            deposit_token_id: accounts(1),
            min_buy: U128(100),
            max_buy: U128(10000),
            max_amount: max_amount.map(|a| U128(a)),
            start_date: U64(start_date),
            end_date: U64(end_date),
            price: U128(1000),
            whitelist_hash: None,
        });
        assert_eq!(contract.get_referral_fees(), referral_fees);
        assert_eq!(contract.get_join_fee(), join_fee);
        (context, contract)
    }

    fn contract_with_sale() -> (VMContextBuilder, Contract) {
        contract_with_sale_info(Some(10000), 0, 1_000_000_000)
    }

    fn register_account(
        context: &mut VMContextBuilder,
        contract: &mut Contract,
        account_id: AccountId,
    ) {
        testing_env!(context
            .predecessor_account_id(account_id)
            .attached_deposit(1000000)
            .build());
        contract.join();
    }

    fn deposit(context: &mut VMContextBuilder, contract: &mut Contract, account_id: AccountId) {
        testing_env!(context.predecessor_account_id(accounts(1)).build());
        contract.ft_on_transfer(
            account_id,
            U128(100),
            serde_json::to_string(&SaleDeposit { sale_id: 0 }).unwrap(),
        );
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
        assert_eq!(contract.get_account(accounts(2)).referrer, accounts(0));

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

        assert_eq!(contract.get_num_accounts(), 2);
        assert_eq!(contract.get_accounts(0, 10).len(), 2);
        assert_eq!(contract.get_account(accounts(2)).referrer, accounts(0));
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

    #[test]
    fn test_create_remove_link() {
        let (mut context, mut contract) = contract_with_sale();
        testing_env!(context
            .predecessor_account_id(accounts(2))
            .attached_deposit(1000000)
            .build());
        contract.join();
        testing_env!(context
            .predecessor_account_id(accounts(2))
            .attached_deposit(CREATE_LINK_AMOUNT)
            .build());
        let pk = PublicKey::from_str("qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz").unwrap();
        contract.create_link(pk.clone());
        contract.remove_link(pk);
    }

    #[test]
    #[should_panic = "ERR_SALE_NOT_STARTED"]
    fn test_sale_too_early() {
        let (mut context, mut contract) = contract_with_sale_info(None, 1_000, 1_000_000);
        register_account(&mut context, &mut contract, accounts(2));
        deposit(&mut context, &mut contract, accounts(2));
    }

    #[test]
    #[should_panic = "ERR_SALE_DONE"]
    fn test_sale_too_late() {
        let (mut context, mut contract) = contract_with_sale_info(None, 1_000, 1_000_000);
        register_account(&mut context, &mut contract, accounts(2));
        testing_env!(context
            .block_timestamp(1_000_001)
            .predecessor_account_id(accounts(1))
            .build());
        contract.ft_on_transfer(
            accounts(2),
            U128(100),
            serde_json::to_string(&SaleDeposit { sale_id: 0 }).unwrap(),
        );
    }
}
