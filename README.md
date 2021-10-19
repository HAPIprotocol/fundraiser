# Fundraising contract

Fundraising contract, with two core features:
 - Create sales, that people can deposit funds into.
 - Create linkdrops for creating accounts. This gets recorded in the internal table of this contract to see which account helped created which account.

## Registering new accounts

As a secondary goal of this fundraising contract is to help onboard more users on NEAR. 
To facilitate this, contract provides existing users a way to create a linkdrop for onboarding users.

Linkdrops are a single usage private key that allows to issue a transaction to the given contract. 
This call will in turn create a new ".near" account and record that this account was paid by user who created linkdrop.   

## Sales distribution process

This contract doesn't handle sales distribution process, instead leaving this to the owner.
Owner will be able to extra the table of all the users and how much they have deposited for the given sale.
After that, owner should airdrop the tokens according to whatever other rules (referral, whitelists, etc).
Referral map of account creations can be extracted as well by listing all the users or queried for specific user.

# Testing

We are going to use `dev-1634658127682-97093903837694` test token as a deposit token.
To mint yourself some of it call:

```
near call dev-1634658127682-97093903837694 mint '{"account_id": "dev-1634657876145-24893242863336", "amount": "10000000000000"}' --accountId dev-1634657876145-24893242863336
```

Next steps dev-deploy the contract:

```
./build_local.sh

near dev-deploy --wasmFile=res/fundraiser_local.wasm

near call dev-1634657876145-24893242863336 new '{"owner_id": "dev-1634657876145-24893242863336", "join_fee": "100000", "referral_fees": [10, 20, 30]}' --accountId dev-1634657876145-24893242863336
```

Replace `dev-1634657876145-24893242863336` with what dev-deploy command will output.

Create new sale with the token above:

```
near call dev-1634657876145-24893242863336 create_sale '{"sale": {"metadata": {"name": "test", "symbol": "TEST", "description": "test", "logo_url": "", "smart_contract_url": ""}, "min_near_deposit": "0", "deposit_token_id": "dev-1634658127682-97093903837694", "min_buy": "1", "max_buy": "10000", "start_date": "10000000", "end_date": "100000000", "price": "1000"}}' --accountId dev-1634657876145-24893242863336
```

View sale info:

```
near view dev-1634657876145-24893242863336 get_sale '{"sale_id": 2}'
```
