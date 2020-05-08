// Copyright 2020 Chris D'Costa
// This file is part of Totem Live Accounting.
// Author Chris D'Costa email: chris.dcosta@totemaccounting.com

// Totem is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Totem is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Totem.  If not, see <http://www.gnu.org/licenses/>.


//********************************************************//
// This is the main Totem Global Accounting Ledger 
//********************************************************//

// It handles all the ledger postings plus some other funky stuff like prefunding.
// The account number follows the chart of accounts definitions and is constructed as a concatenation of:
// Financial Statement Type Number int length 1 (Mainly Balance Sheet, Profit and Loss, and Memorandum) 
// Account Category Number int length 1 (Mainly Assets, liabilities, Equity, Revenue and Expense, and non-balance sheet) 
// Account Category Group number int length 1 (e.g. grouping expenses: operating expense, other opex, personnel costs)
// Accounting Group Nr concatenation of int length 4 + int length 4. The first four digits incrementing within the Category Group (e.g. range 1000-1999) for individual Accounting Group values 
// associated with the Category Group Number. The second four digits incrementing within the group (e.g. range 10001000-10001999) for individual Accounting Groups within the group itself.
// The last 4 ints are the Accounting Subgroup Number which specify where the value is posted.

// For example 250500120000011
// Statement Type: Profit and Loss (2)
// Account Category: Expenses (5) 
// Account Category Grp: Operating Expenses (0), 
// Accounting Group: Services (50012000), 
// Accounting Subgroup: Technical Assitance (0011)

// In other accounting systems there are further values hierarchically below the subgroup (for example if you have multiple bank accounts), but this is not necessary in Totem as this is
// replaced by the notion of Identity. The key takeaway is that everything in Totem is a property of an Identity

// For example in reporting you may drill down to the detail in a heirarchical report like this:
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > CitiCorp Account (Identity)
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > Bank of America Account (Identity)
// Here the Ledger Account has a 1:n relationship to the identities, and therefore aggregates results

// But actually this is just the rearrangement of the attributes (or properties of an individual identity 
// CitiCorp Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000 
// Bank of America Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000 
// Here the Identity has a 1:1 relationship to its properties

//********************************************************//
use parity_codec::{Encode};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap};
use runtime_primitives::traits::Hash; // Use with node template only
// use node_primitives::Hash; // Use with full node
use system::{self, ensure_signed};
use rstd::prelude::*;

pub type AccountBalance = i64; // Balance on an account can be negative - needs to be larger than the 
pub type Account = u64; // General ledger account number
pub type Amount = u64; // Amount being transferred - check this should only be the amount 
pub type Indicator = bool; // 0=Debit(false) 1=Credit(true) Note: Debit and Credit balances are account specific - see chart of accounts
pub type UnLocked = bool; // 0=Unlocked(false) 1=Locked(true)
pub type Symbol = Vec<u8>; // Currency Symbol 
pub type Rate = u32; // Exchange Rate
pub type Status = u16; // Generic Status for whatever the HashReference refers to
pub type Deadline = u64; // Deadline as blocknumber

pub trait Trait: balances::Trait + system::Trait + timestamp::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// Key : Account Identity and General Ledger Account Number 
// Value : General Ledger Account Number (corresponding debit or credit), Current Blocknumber, Corresponding Account Identity, prior Balance, new balance, amount, debit or credit indicator
// Rather than storing detail at the record

decl_storage! {
    trait Store for Module<T: Trait> as TotemModule {
        // Accounting Balances 
        BalanceByLedger get(balance_by_ledger): map (T::AccountId, Account) => AccountBalance;
        // Detail of the accounting posting (for audit)
        Detail get(detail): map (T::AccountId, Account) => Option<(T::BlockNumber,AccountBalance,Indicator,T::Hash)>;
        // Address to book the sales tax to, and the tax jurisdiction 
        SalesTax get(sales_tax): map (T::AccountId, T::AccountId) => AccountBalance;

        // yay! Totem!
        GlobalLedger get(global_ledger): map Account => AccountBalance;
        
        // Funds Storage on Prefunding
        // This storage is intended to signal to a marketplace that the originator is prepared to lockup funds to a deadline.
        // If the sender accepts respondence then the funds are moved to the main prefunding account
        // After deadline sender can withdraw funds
        Prefunding get(prefunding): map T::Hash => Option<(AccountBalance,Deadline)>;
        
        // Says who can take the money after deadline. Includes intended owner (same as origin for market posting)
        // 10, sender can take after deadline (initial state)
        // 11, accepted by recipient. (funds locked, nobody can take) 
        // 01, sender approves (recipient can take, or refund)
        // 00, only the recipient authrises sender to retake funds regardless of deadline.
        PrefundingHashOwner get(prefunding_hash_owner): map T::Hash => Option<(T::AccountId, UnLocked, T::AccountId, UnLocked)>;
        
        // List for convenience
        OwnerPrefundingHashList get(owner_prefunding_hash_list): map T::AccountId => Vec<T::Hash>;
        
        // Reference Hash generic status
        ReferenceStatus get(reference_status): map T::Hash => Option<Status>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}

impl<T: Trait> Module<T> {
    // Basic posting function (warning! can cause imbalance if not called with corresponding entries)
    // The reason why this is a simple function is that (for example) one debit posting may correspond with one or many credit
    // postings and vice-versa. For example Accounts Receivable is the gross invoice amount, which could correspond with 
    // a credit to liabilities for the sales tax amount and a credit to revenue for the net invoice amount. The sum of both credits being 
    // equal to the single posting into accounts receivable, but only one posting needs to be made to that account, and two posting for the others.
    fn post(i: T::AccountId, a: Account, c: AccountBalance, d: bool, r: T::Hash) -> Result {
        let zero: AccountBalance = 0;
        let key = (i.clone(), a);
        let current_block = <system::Module<T>>::block_number();
        let abs: AccountBalance = c.abs();
        let detail = (current_block, abs, d, r);
        
        // !! Warning !! 
        // Values could feasibly overflow, with no visibility on other accounts. Therefore need to handle the error states
        // Reversals occur in the function that calls this function, but updates are only made to storage once all three tests are passed
        if c > zero {
            // check adding the new amount to the existing balance
            match Self::balance_by_ledger(&key).checked_add(c) {
                None => {
                    Self::deposit_event(RawEvent::ErrorOverflow(a));
                    return Err("Balance Value overflowed");
                },
                Some(n) => {
                    match Self::global_ledger(&a).checked_add(c) {
                        Some(n) => (),
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow(a));
                            return Err("Balance Value overflowed");
                        },
                    }
                },
            };
        } else if c < zero {
            // check subtracting the new amount from the existing balance
            match Self::balance_by_ledger(&key).checked_sub(c) {
                None => {
                    Self::deposit_event(RawEvent::ErrorOverflow(a));
                    return Err("Balance Value overflowed");
                },
                Some(n) => {
                    match Self::global_ledger(&a).checked_sub(c) {
                        Some(n) => (),
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow(a));
                            return Err("Balance Value overflowed");
                        },
                    }
                },
            };
        }
        
        <BalanceByLedger<T>>::mutate(&key, |v| *v += c);
        <Detail<T>>::insert(&key, detail);
        <GlobalLedger<T>>::mutate(&a, |v| *v += c);
        
        Self::deposit_event(RawEvent::LegderUpdate(i, a, c));
        
        Ok(())
    }

    fn ensure_absolute(amount: AccountBalance) -> AccountBalance {
        let zero: AccountBalance = 0; 
        let absolute_amount: AccountBalance;        
        
        if amount < zero {
            absolute_amount = amount * -1;    
        } else {
            absolute_amount = amount;
        }
    
        return absolute_amount;
    }
    
    fn prefunding_for(origin: T::Origin, recipient: T::AccountId, amount: AccountBalance, deadline: Deadline) -> Result {
        
        let who = ensure_signed(origin)?;
        // amount cannot be negative
        let increase_amount: AccountBalance = Self::ensure_absolute(amount);
        let decrease_amount: AccountBalance = -Self::ensure_absolute(amount);
    
        // manage the deposit
        let reference: T::Hash = Self::set_prefunding(who.clone(), recipient.clone(), amount, deadline); 
        
        // TODO Set status
    
        // Process Balance Sheet and P&L updates
        // debit increase 110100050000000 Prefunding Account
        let mut account: Account = 110100050000000;
        // This is the first posting, if it errors there is nothing to reverse. Then exit.
        match Self::post(who.clone(), account, increase_amount, false, reference) {
            Ok(_) => (),
            Err(_) => return Err("Overflow error, amount too big!"),
        }
        // Self::post(who.clone(), account, increase_amount, false, reference)?;
        
        // credit decrease 110100040000000 XTX Balance
        account = 110100040000000;
        match Self::post(who.clone(), account, decrease_amount, true, reference) {
            Ok(_) => (),
            Err(_) => {
                // Error before the value was updated. Need to reverse-out the earlier debit amount and account combination
                // as this has already changed in storage.
                account = 110100050000000;
                Self::post(who.clone(), account, decrease_amount, true, reference);
                return Err("Overflow error, amount too big!");
            },
        }

        // Update Memorandum Ledgers
        // debit increase 360600020000000	Runtime Ledger by Module
        account = 360600020000000;
        match Self::post(who.clone(), account, increase_amount, false, reference) {
            Ok(_) => (),
            Err(_) => {
                // Error before the value was updated. Need to reverse-out the earlier credits and debits
                // as these values has already changed in storage.
                account = 110100050000000;
                Self::post(who.clone(), account, decrease_amount, true, reference);
                account = 110100040000000;
                Self::post(who.clone(), account, increase_amount, false, reference);
                return Err("Overflow error, amount too big!");
            },
        }
        
        // debit increase 360600060000000	Runtime Ledger Control
        account = 360600060000000;
        match Self::post(who.clone(), account, increase_amount, false, reference) {
            Ok(_) => (),
            Err(_) => {
                // Error before the value was updated. Need to reverse-out the earlier credits and debits
                // as these values has already changed in storage.
                account = 110100050000000;
                Self::post(who.clone(), account, decrease_amount, true, reference);
                account = 110100040000000;
                Self::post(who.clone(), account, increase_amount, false, reference);
                account = 360600020000000;
                Self::post(who.clone(), account, decrease_amount, true, reference);
                return Err("Overflow error, amount too big!");
            },
        }
        
        Ok(())
    }
        
    fn set_prefunding(s: T::AccountId, r: T::AccountId, c: AccountBalance, d: Deadline) -> T::Hash {
        
        // Prepare
    
        // TODO Ensure that the funds can be substrated from sender's balance 
        // without causing the account to be destroyed by the existential deposit 
        
        let prefunding_hash: T::Hash = Self::get_pseudo_random_value(s.clone(), r.clone());
        
        // let abs: T::Balance = Self::ensure_absolute(&c);
    
        // secure funds
        // <balances::Module<T>>::decrease_free_balance(&s, &abs);
        
        // store in runtime
        let prefunded = (Self::ensure_absolute(c), d);
        let owners = (s.clone(), true, r.clone(), false);
        <Prefunding<T>>::insert(&prefunding_hash, prefunded);
        <PrefundingHashOwner<T>>::insert(prefunding_hash, owners); 
        
        // add hash to list
        <OwnerPrefundingHashList<T>>::mutate(&s, |owner_prefunding_hash_list| {
            owner_prefunding_hash_list.push(prefunding_hash)
        });
        
        Self::deposit_event(RawEvent::PrefundingDeposit(s, c, d));
    
        return prefunding_hash;
    }
    
    fn get_pseudo_random_value(sender: T::AccountId, recipient: T::AccountId) -> T::Hash {
        let tuple = (sender, recipient);
        let input = (
            tuple,
            <timestamp::Module<T>>::get(),
            <system::Module<T>>::random_seed(),
            <system::Module<T>>::extrinsic_index(),
            <system::Module<T>>::block_number()
        );
        return T::Hashing::hash(input.encode().as_slice()); // default hash BlakeTwo256
    }

}

decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    {
        LegderUpdate(AccountId, Account, AccountBalance),
        PrefundingDeposit(AccountId, AccountBalance, Deadline),
        ErrorOverflow(Account),
    }
);

//     // Simple invoice. Does not include tax jurisdiction, tax amounts, freight, commissions, tariffs, discounts and other extended line item values
//     // must include a connection to the original hash
//     fn simple_invoice(origin, payer: T::AccountId, net: AccountBalance, reference: Hashreference) -> Result {
        
//         // Validate that the hash is indeed assigned to the seller

//         // Seller Origin
//         // Debit increase 110100080000000	Accounts receivable (Sales Control Account or Trade Debtor's Account)
//         // Credit increase 240400010000000	Product or Service Sales
//         // Debit increase 360600010000000	Sales Ledger by Payer
//         // Debit increase 360600050000000	Sales Ledger Control

//         // Buyer Payer
//         // Credit increase 120200030000000	Accounts payable
//         // Debit increase 250500120000013	Labour
//         // Debit increase 360600030000000	Purchase Ledger by Vendor
//         // Debit increase 360600070000000	Purchase Ledger Control

//         // Add status processing

//         Ok(());
//     }

//     // This takes a simple invoice and settles it.
//     fn simple_prefunded_settlement() -> Result {
//         // validate the status of the invoice.
// // Buyer
// //         Debit	120200030000000	Accounts payable
// // Credit	110100050000000	Totem Runtime Deposit (Escrow)
// // Credit	360600020000000	Runtime Ledger by Module
// // Credit	360600060000000	Runtime Ledger Control
// // Credit	360600030000000	Purchase Ledger by Vendor
// // Credit	360600070000000	Purchase Ledger Control

// // Seller
// // Debit	n/a	110100040000000	XTX Balance
// // n/a	Credit	110100080000000	Accounts receivable (Sales Control Account or Trade Debtor's Account)
// // n/a	Credit	360600010000000	Sales Ledger by Payer
// // n/a	Credit	360600050000000	Sales Ledger Control

// Ok(());
//     }


// // impl<T: Trait> ContractAddressFor<CodeHash<T>, T::AccountId> for SimpleAddressDeterminator<T>
// impl<T: Trait> Posting<T::AccountId,Account> for Module<T> {
//     fn get_ledger_balance(who: T::AccountId, ledger: Account) -> Self::AccountBalance {    
//         <BalanceByLedger<T>>::get(&who, ledger);
//     }

//     fn get_detail(who: T::AccountId, ledger: Account) -> Self::AccountBalance {    
//         <Detail<T>>::get(&who, ledger);
//     }

// }