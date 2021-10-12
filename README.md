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