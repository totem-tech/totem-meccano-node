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

// It handles all the ledger postings.
// The account number follows the chart of accounts definitions and is constructed as a concatenation of:
// * Financial Statement Type Number int length 1 (Mainly Balance Sheet, Profit and Loss, and Memorandum) 
// * Account Category Number int length 1 (Mainly Assets, liabilities, Equity, Revenue and Expense, and non-balance sheet) 
// * Account Category Group number int length 1 (e.g. grouping expenses: operating expense, other opex, personnel costs)
// * Accounting Group Nr concatenation of int length 4 + int length 4. The first four digits incrementing within the Category Group (e.g. range 1000-1999) for individual Accounting Group values 
// associated with the Category Group Number. The second four digits incrementing within the group (e.g. range 10001000-10001999) for individual Accounting Groups within the group itself.
// * The last 4 ints are the Accounting Subgroup Number which specify where the value is posted.

// For example 250500120000011
// Statement Type: Profit and Loss (2)
// Account Category: Expenses (5) 
// Account Category Grp: Operating Expenses (0), 
// Accounting Group: Services (50012000), 
// Accounting Subgroup: Technical Assitance (0011)

// In other accounting systems there are further values hierarchically below the subgroup (for example if you have multiple bank accounts), but this is not necessary in Totem as this is
// replaced by the notion of Identity. The key takeaway is that everything in Totem is a property of an Identity

// For example in reporting yAmount_ou may drill down to the detail in a heirarchical report like this:
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > CitiCorp Account (Identity)
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > Bank of America Account (Identity)
// Here the Ledger Account has a 1:n relationship to the identities, and therefore aggregates results

// In fact this is just the rearrangement of the attributes (or properties of an individual identity 
// CitiCorp Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000 
// Bank of America Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000 
// Here the Identity has a 1:1 relationship to its properties defined in the account number that is being posted to

// use parity_codec::{Decode, Encode, Codec};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap};
use system::{self};
use rstd::prelude::*;

// Totem Traits
use crate::totem_traits::{ Posting };

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type AccountBalance = i128; // Balance on an account can be negative - needs to be larger than the 
type Account = u64; // General ledger account number
type Indicator = bool; // 0=Debit(false) 1=Credit(true) Note: Debit and Credit balances are account specific - see chart of accounts

decl_storage! {
    trait Store for Module<T: Trait> as AccountingModule {
        // Accounting Balances 
        BalanceByLedger get(balance_by_ledger): map (T::AccountId, Account) => AccountBalance;
        // Detail of the accounting posting (for Audit)
        AmountDetail get(amount_detail): map (T::AccountId, Account) => Option<(T::BlockNumber,AccountBalance,Indicator,T::Hash, T::BlockNumber)>;
        // yay! Totem!
        GlobalLedger get(global_ledger): map Account => AccountBalance;
        // Address to book the sales tax to and the tax jurisdiction (Experimental, may be deprecated in future) 
        TaxesByJurisdiction get(taxes_by_jurisdiction): map (T::AccountId, T::AccountId) => AccountBalance;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}

impl<T: Trait> Module<T> {
    /// Basic posting function (warning! can cause imbalance if not called with corresponding debit or credit entries)
    /// The reason why this is a simple function is that (for example) one debit posting may correspond with one or many credit
    /// postings and vice-versa. For example a debit to Accounts Receivable is the gross invoice amount, which could correspond with 
    /// a credit to liabilities for the sales tax amount and a credit to revenue for the net invoice amount. The sum of both credits being 
    /// equal to the single debit in accounts receivable, but only one posting needs to be made to that account, and two posting for the others.
    /// The Totem Accounting Recipes are constructed using this simple function.
    /// The second Blocknumber is for re-targeting the entry in the accounts, i.e. for adjustments prior to or after the current period (generally accruals).
    fn post_amounts((o, a, c, d, h, b, t): (T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber, T::BlockNumber)) -> Result {
        let zero: AccountBalance = 0;
        let key = (o.clone(), a);
        let ab: AccountBalance = c.abs();
        let detail = (b, ab, d, h, t);
        
        // !! Warning !! 
        // Values could feasibly overflow, with no visibility on other accounts. In this even this function return an error.
        // Reversals must occur in the parent function (that calls this function). Updates are only made to storage once all three tests below are passed for debits or credits.
        if c > zero {
            // check adding the new amount to the existing balance
            match Self::balance_by_ledger(&key).checked_add(c) {
                None => {
                    Self::deposit_event(RawEvent::ErrorOverflow(a));
                    return Err("Balance Value overflowed");
                },
                Some(_) => {
                    match Self::global_ledger(&a).checked_add(c) {
                        Some(_) => (),
                        None => {
                            Self::deposit_event(RawEvent::ErrorGlobalOverflow());
                            return Err("Global Balance Value overflowed");
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
                Some(_) => {
                    match Self::global_ledger(&a).checked_sub(c) {
                        Some(_) => (),
                        None => {
                            Self::deposit_event(RawEvent::ErrorGlobalOverflow());
                            return Err("Global Balance Value overflowed");
                        },
                    }
                },
            };
        }
        
        <BalanceByLedger<T>>::mutate(&key, |v| *v += c);
        <AmountDetail<T>>::insert(&key, detail);
        <GlobalLedger<T>>::mutate(&a, |v| *v += c);
        
        Self::deposit_event(RawEvent::LegderUpdate(o, a, c));
        
        Ok(())
    }
}

// impl<T: Clone + Decode + Encode + Codec + Eq> Posting<T::AccountId,T::Hash,T::BlockNumber> for Module<T> {
impl<T: Trait> Posting<T::AccountId,T::Hash,T::BlockNumber> for Module<T> {
    
    type Account = Account;
    type AccountBalance = AccountBalance;
    
    /// The Totem Accounting Recipes are constructed using this function which handles posting to multiple accounts.
    /// It is exposed to other modules as a trait
    /// If for whatever reason an error occurs during the storage processing which is sequential
    /// this function also handles reversing out the prior accounting entries
    /// Therefore the recipes that are passed as arguments need to be be accompanied with a reversal
    /// Obviously the last posting does not need a reversal for if it errors, then it was not posted in the first place.
    fn handle_multiposting_amounts(
        // o: <T as system::Trait>::AccountId,
        o: T::AccountId,
        fwd: Vec<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber, T::BlockNumber)>, 
        rev: Vec<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber, T::BlockNumber)>, 
        trk: Vec<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber, T::BlockNumber)>) -> Result {
            
            let reversal_keys = rev.clone();
            let mut track_rev_keys = trk.clone();
            let length_limit = reversal_keys.len();
            
            // Iterate over forward keys. If Ok add reversal key to tracking, if error, then reverse out prior postings.
            for (pos, a) in fwd.clone().iter().enumerate() {
                
                match Self::post_amounts(a.clone()) {
                    Ok(_) => { 
                        if pos < length_limit { track_rev_keys.push(reversal_keys[pos].clone()) };
                    },
                    Err(_) => {
                        // Error before the value was updated. Need to reverse-out the earlier debit amount and account combination
                        // as this has already changed in storage.
                        for (_dummy_pos, b) in track_rev_keys.iter().enumerate() {
                            match Self::post_amounts(b.clone()) {
                                Ok(_) => (),
                                Err(_) => {
                                    // This event is because there is a major system error in the reversal process
                                    Self::deposit_event(RawEvent::ErrorInError(o));
                                    return Err("System Failure in Account Posting");
                                },
                            }
                        }
                        Self::deposit_event(RawEvent::ErrorOverflow(a.1));
                        return Err("Overflow error, amount too big!");
                    },
                }
            }
        Ok(())
    }
}
    
decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    {
        LegderUpdate(AccountId, Account, AccountBalance),
        ErrorOverflow(Account),
        ErrorGlobalOverflow(),
        ErrorInsufficientFunds(AccountId),
        ErrorInError(AccountId),
    }
);