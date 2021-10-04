use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{AccountId, Balance, CryptoHash, PromiseOrValue, assert_self, log};

use crate::*;

/// Sale information for creating new sale.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleInput {
    pub staking_contract: Option<AccountId>,
    pub min_near_deposit: U128,
    pub deposit_token_id: AccountId,
    pub min_buy: U128,
    pub max_buy: U128,
    pub max_amount: U128,
    pub price: U128,
    pub whitelist_hash: Option<CryptoHash>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleOutput {
    pub staking_contract: Option<AccountId>,
    pub min_near_deposit: U128,
    pub deposit_token_id: AccountId,
    pub min_buy: U128,
    pub max_buy: U128,
    pub max_amount: U128,
    pub price: U128,
    pub whitelist_hash: Option<CryptoHash>,
    pub collected_amount: U128,
    pub num_account_sales: u64,
}

/// Sale information.
#[derive(BorshSerialize, BorshDeserialize)]
pub enum VSale {
    Current(Sale),
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Sale {
    pub staking_contract: Option<AccountId>,
    pub min_near_deposit: Balance,
    pub deposit_token_id: AccountId,
    pub min_buy: Balance,
    pub max_buy: Balance,
    pub max_amount: Balance,
    pub price: Balance,
    pub whitelist_hash: Option<CryptoHash>,

    pub collected_amount: Balance,
    pub account_sales: UnorderedMap<AccountId, VSaleAccount>,
}

impl From<VSale> for Sale {
    fn from(v_sale: VSale) -> Self {
        match v_sale {
            VSale::Current(sale) => sale,
        }
    }
}

impl From<VSale> for SaleOutput {
    fn from(v_sale: VSale) -> Self {
        match v_sale {
            VSale::Current(sale) => SaleOutput {
                staking_contract: sale.staking_contract,
                min_near_deposit: U128(sale.min_near_deposit),
                deposit_token_id: sale.deposit_token_id,
                min_buy: U128(sale.min_buy),
                max_buy: U128(sale.max_buy),
                max_amount: U128(sale.max_amount),
                price: U128(sale.price),
                whitelist_hash: sale.whitelist_hash,
                collected_amount: U128(sale.collected_amount),
                num_account_sales: sale.account_sales.keys_as_vector().len(),
            },
        }
    }
}

impl VSale {
    pub fn new(sale_id: u64, sale_input: SaleInput) -> Self {
        Self::Current(Sale {
            staking_contract: sale_input.staking_contract,
            min_near_deposit: sale_input.min_near_deposit.0,
            deposit_token_id: sale_input.deposit_token_id,
            min_buy: sale_input.min_buy.0,
            max_buy: sale_input.max_buy.0,
            max_amount: sale_input.max_amount.0,
            price: sale_input.price.0,
            whitelist_hash: sale_input.whitelist_hash,
            collected_amount: 0,
            account_sales: UnorderedMap::new(StorageKey::AccountSales { sale_id }),
        })
    }
}

/// Account deposits for the a sale.
#[derive(BorshSerialize, BorshDeserialize)]
pub enum VSaleAccount {
    Current(SaleAccount),
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct SaleAccount {
    pub amount: U128,
}

impl From<VSaleAccount> for SaleAccount {
    fn from(v_account_sale: VSaleAccount) -> Self {
        match v_account_sale {
            VSaleAccount::Current(account_sale) => account_sale,
        }
    }
}

impl Contract {
    /// Validates deposit and records it for the given user for give sale.
    /// Returns extra amount if sale is already over capacity.
    pub(crate) fn internal_sale_deposit(
        &mut self,
        sale_id: u64,
        token_id: &AccountId,
        sender_id: &AccountId,
        staked_amount: Balance,
        amount: Balance,
    ) -> Balance {
        let mut sale: Sale = self.sales.get(&sale_id).expect("ERR_NO_SALE").into();
        assert_eq!(&sale.deposit_token_id, token_id, "ERR_WRONG_TOKEN");
        assert!(
            staked_amount >= sale.min_near_deposit,
            "ERR_NOT_ENOUGH_STAKED"
        );
        // TODO: add check for the whitelist.
        let deposit_amount = std::cmp::min(amount, sale.max_amount - sale.collected_amount);
        let mut account_sale = sale
            .account_sales
            .get(&sender_id)
            .map(|account_sale| account_sale.into())
            .unwrap_or(SaleAccount { amount: U128(0) });
        account_sale.amount = U128(account_sale.amount.0 + deposit_amount);
        assert!(
            sale.max_buy >= account_sale.amount.0 && sale.min_buy <= account_sale.amount.0,
            "ERR_WRONG_AMOUNT"
        );
        sale.account_sales
            .insert(&sender_id, &VSaleAccount::Current(account_sale));
        sale.collected_amount += deposit_amount;
        self.sales.insert(&sale_id, &VSale::Current(sale));
        amount - deposit_amount
    }
}

#[near_bindgen]
impl Contract {
    pub fn create_sale(&mut self, sale: SaleInput) {
        assert_eq!(
            self.owner_id,
            env::predecessor_account_id(),
            "ERR_MUST_BE_OWNER"
        );
        self.sales
            .insert(&self.num_sales, &VSale::new(self.num_sales, sale));
        self.num_sales += 1;
    }

    pub fn get_num_sales(&self) -> u64 {
        self.num_sales
    }

    pub fn get_sale(&self, sale_id: u64) -> SaleOutput {
        self.sales.get(&sale_id).expect("ERR_NO_SALE").into()
    }

    pub fn get_sales(&self, from_index: u64, limit: u64) -> Vec<SaleOutput> {
        (from_index..std::cmp::min(from_index + limit, self.num_sales))
            .filter_map(|sale_id| self.sales.get(&sale_id).map(|sale| sale.into()))
            .collect()
    }

    pub fn get_sale_accounts(
        &self,
        sale_id: u64,
        from_index: u64,
        limit: u64,
    ) -> Vec<(AccountId, SaleAccount)> {
        let sale: Sale = self.sales.get(&sale_id).expect("ERR_NO_SALE").into();
        let keys = sale.account_sales.keys_as_vector();
        let values = sale.account_sales.values_as_vector();
        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| (keys.get(index).unwrap(), values.get(index).unwrap().into()))
            .collect()
    }

    pub fn get_sale_account(&self, sale_id: u64, account_id: AccountId) -> SaleAccount {
        let sale: Sale = self.sales.get(&sale_id).expect("ERR_NO_SALE").into();
        sale.account_sales
            .get(&account_id)
            .expect("ERR_NO_ACCOUNT_SALE")
            .into()
    }

    pub fn on_get_account_staked_balance(
        &mut self,
        #[callback] staked_amount: U128,
        sale_id: u64,
        token_id: AccountId,
        sender_id: AccountId,
        deposit_amount: U128,
    ) -> PromiseOrValue<U128> {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(), "ERR_NOT_OWNER");
        log!("{} stake: {}", sender_id, staked_amount.0);
        PromiseOrValue::Value(U128(self.internal_sale_deposit(
            sale_id,
            &token_id,
            &sender_id,
            staked_amount.0,
            deposit_amount.0,
        )))
    }
}
