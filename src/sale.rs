use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{AccountId, Balance, CryptoHash, PromiseOrValue, log, Timestamp};

use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleMetadata {
    /// Project name that is going to be on sale.
    pub name: String,
    /// Symbol (ticker) for the token on sale.
    pub symbol: String,
    /// Project description.
    pub description: String,
    /// Link to project smart contract.
    pub smart_contract_url: String,
    /// Project logo.
    pub logo_url: String,
}

/// Sale information for creating new sale.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleInput {
    pub metadata: SaleMetadata,
    /// Staking contract that will be checked if user has staked with it.
    pub staking_contract: Option<AccountId>,
    /// Minimum NEAR staked in the above staking contract.
    pub min_near_deposit: U128,
    /// Token to sell for.
    pub deposit_token_id: AccountId,
    /// Minimum amount of deposit token.
    pub min_buy: U128,
    /// Maximum amount of deposit token for one account.
    pub max_buy: U128,
    /// Maximum amount that can be collected by the sale.
    pub max_amount: Option<U128>,
    /// Start date of the sale.
    pub start_date: U64,
    /// End date of the sale.
    pub end_date: U64,
    /// Price per a single token in decimals of the deposit token.
    pub price: U128,
    /// Hash of the merkle tree of whitelisted accounts.
    pub whitelist_hash: Option<CryptoHash>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleOutput {
    pub metadata: SaleMetadata,
    pub staking_contract: Option<AccountId>,
    pub min_near_deposit: U128,
    pub deposit_token_id: AccountId,
    pub min_buy: U128,
    pub max_buy: U128,
    pub max_amount: Option<U128>,
    pub start_date: U64,
    pub end_date: U64,
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
    pub metadata: SaleMetadata,
    pub staking_contract: Option<AccountId>,
    pub min_near_deposit: Balance,
    pub deposit_token_id: AccountId,
    pub min_buy: Balance,
    pub max_buy: Balance,
    pub max_amount: Option<Balance>,
    pub start_date: Timestamp,
    pub end_date: Timestamp,
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
                metadata: sale.metadata,
                staking_contract: sale.staking_contract,
                min_near_deposit: U128(sale.min_near_deposit),
                deposit_token_id: sale.deposit_token_id,
                min_buy: U128(sale.min_buy),
                max_buy: U128(sale.max_buy),
                max_amount: sale.max_amount.map(|amount| U128(amount)),
                start_date: U64(sale.start_date),
                end_date: U64(sale.end_date),
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
            metadata: sale_input.metadata,
            staking_contract: sale_input.staking_contract,
            min_near_deposit: sale_input.min_near_deposit.0,
            deposit_token_id: sale_input.deposit_token_id,
            min_buy: sale_input.min_buy.0,
            max_buy: sale_input.max_buy.0,
            max_amount: sale_input.max_amount.map(|amount| amount.0),
            start_date: sale_input.start_date.0,
            end_date: sale_input.end_date.0,
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

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
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
        // TODO: add check for the whitelist hash.
        let deposit_amount = if let Some(max_amount) = sale.max_amount {
            std::cmp::min(amount, max_amount - sale.collected_amount)
        } else {
            amount
        };
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
    pub fn create_sale(&mut self, sale: SaleInput) -> u64 {
        assert_eq!(
            self.owner_id,
            env::predecessor_account_id(),
            "ERR_MUST_BE_OWNER"
        );
        self.sales
            .insert(&self.num_sales, &VSale::new(self.num_sales, sale));
        self.num_sales += 1;
        self.num_sales
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
