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

//********************************************************//
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap};
use node_primitives::Hash;
use substrate_primitives::H256;
// use system::ensure_signed;
use system::{self, ensure_signed};
use rstd::prelude::*;
use balances;

pub type AccountBalance = i128; // Balance on an account can be negative - needs to be larger than the 
pub type Account = u64; 
pub type Amount = u64; // Amount being transferred - check this should only be the amount 
pub type Indicator = bool; // 0=Debit 1=Credit Note: Debit and Credit balances are account specific - see chart of accounts
pub type Lock = bool; // 0=Unlocked 1=Locked
pub type Deadline = BlockNumber; // Deadline
pub type Symbol = Vec<u8>; // Currency Symbol 
pub type Rate = u32; // Exchange Rate
pub type Hashreference = H256; // generic Hash Reference
pub type Status = u16;

pub trait Trait: balances::Trait + system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

/// Key : Account Identity and General Ledger Account Number 
/// Value : General Ledger Account Number (corresponding debit or credit), Current Blocknumber, Corresponding Account Identity, prior Balance, new balance, amount, debit or credit indicator
/// Rather than storing detail at the record

decl_storage! {
    trait Store for Module<T: Trait> as TotemModule {
        // Accounting Balances 
        BalanceByLedger get(balance_by_ledger): map (T::AccountId, Account) => Option<AccountBalance>;
        // Detail of the accounting posting (for audit)
        Detail get(detail): map (T::AccountId, Account) => Option<(T::BlockNumber,AccountBalance,Indicator,Hashreference)>;
        // Address to book the sales tax to, and the tax jurisdiction 
        SalesTax get(sales_tax): map (T::Account, T::Account) => Option<AccountBalance>;

        // yay! Totem
        GlobalLedger get(global_ledger): map (Account) => Option<AccountBalance>;
        
        // Funds Storage on Prefunding
        // This storage is intended to signal to a marketplace that the originator is prepared to lockup funds to a deadline.
        // If the sender accepts respondence then the funds are moved to the main prefunding account
        // After deadline sender can withdraw funds
        Prefunding get(prefunding): map PrefundingHash => Option<(AccountBalance,Deadline)>;
        
        // Says who can take the money after deadline. Includes intended owner (same as origin for market posting)
        // 10, sender can take after deadline (initial state)
        // 11, accepted by recipient. (funds locked, nobody can take) 
        // 01, sender approves (recipient can take, or refund)
        // 00, only the recipient authrises sender to retake funds regardless of deadline.
        PrefundingHashOwner get(prefunding_hash_owner): map PrefundingHash => Option<T::AccountId, Lock, T::AccountId, Lock>;
        
        // List for convenience
        OwnerPrefundingHashList get(owner_prefunding_hash_list): map T::AccountId => Vec<PrefundingHash>;
        
        // Reference Hash generic status
        ReferenceStatus get(reference_status) map PrefundingHash => Option<Status>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        fn prefunding_for(origin, recipient: T::Account, amount: Amount, currency: Symbol, rate: Rate, deadline: Deadline) -> Result;
    }
}

impl<T: Trait> Module<T> {
    // Basic posting function (debits and credits)
    fn post(i: T::AccountId, a: Account, c: AccountBalance, d: bool, r: Hashreference) -> Result {
        
        let key = (i, a);
        let current_block = <system::Module<T>>::block_number();
        <BalanceByLedger<T>>::mutate(&key, |v| *v += c);
        <GlobalLedger<T>>::mutate(&a, |v| *v += c);
        // could also just be an event - but events are ephemeral
        let abs: AccountBalance = c.abs();
        <Detail<T>>::insert(&key, current_block, abs, d, r);
        
        Self::deposit_event(RawEvent::LegderUpdate(i, a, c));
        
        OK(());
    }
 
    // Simple invoice. Does not include tax jurisdiction, tax amounts, freight, commissions, tariffs, discounts and other extended line item values
    // must include a connection to the original hash
    fn simple_invoice(origin, payer: T::AccountId, net: AccountBalance, reference: Hashreference) -> Result {
        
        // Seller Origin
        // Debit increase 110100080000000	Accounts receivable (Sales Control Account or Trade Debtor's Account)
        // Credit increase 240400010000000	Product or Service Sales
        // Debit increase 360600010000000	Sales Ledger by Payer
        // Debit increase 360600050000000	Sales Ledger Control

        // Buyer Payer
        // Credit increase 120200030000000	Accounts payable
        // Debit increase 250500120000013	Labour
        // Debit increase 360600030000000	Purchase Ledger by Vendor
        // Debit increase 360600070000000	Purchase Ledger Control


        Ok(());
    }

    fn prefunding_for(
        origin,
        recipient: T::Account,
        amount: Amount,
        deadline: Deadline) -> Result {
        
            let who = ensure_signed(origin)?;
            let increase_amount += amount.into();
            let decrease_amount -= amount.into();

            // manage the deposit
            let reference: T::Hash = Self::set_prefunding(s: T::AccountId, r: T::Account, c: AccountBalance, d: Deadline)?; 
            
            // Process Balance Sheet and P&L updates
            // debit increase 110100050000000 Prefunding Account
            let mut account: Account = 110100050000000;
            Self::post(&who, &account, &increase_amount, 0, &reference)?;
            
            // credit decrease 110100040000000 XTX Balance
            account = 110100040000000;
            Self::post(&who, &account, &decrease_amount, 1, &reference)?;
            
            // Update Memorandum Ledgers
            // debit increase 360600020000000	Runtime Ledger by Module
            account = 360600020000000;
            Self::post(&who, &account, &increase_amount, 0, &reference)?;
            
            // debit increase 360600060000000	Runtime Ledger Control
            account = 360600060000000;
            Self::post(&who, &account, &increase_amount, 0, &reference)?;            
            
            OK(());
        }
        
        
        fn set_prefunding(s: T::AccountId, r: T::Account, c: AccountBalance, d: Deadline) -> T::Hash {
            
            // Prepare
            let prefunding = (&s, &r);
            let prefunding_hash T::Hash = Self::get_pseudo_random_value(&s, &r);
            
            // ensure a positive amount 
            let abs: T::Balance  = c.abs();
            
            // secure funds
            <balances::Module<T>>::decrease_free_balance(&s, &abs);
            
            // store in runtime
            <Prefunding<T>>::insert(&prefunding_hash, abs, d);
            <PrefundingHashOwner<T>>::insert(&prefunding_hash, &s, 1, &r, 0); 
            
            // add hash to list
            <OwnerPrefundingList<T>>::mutate(&s, |owner_prefunding_list| {
                owner_prefunding_list.push(&prefunding_hash)
            });
            
            Self::deposit_event(RawEvent::RuntimeSecurity(s, c, d));

            return prefunding_hash;
        }
        
        fn get_pseudo_random_value(sender: T::AccountId, recipient: T::Account) -> T::Hash {
            let input = (
                <timestamp::Module<T>>::get(),
                <system::Module<T>>::random_seed(),
                (sender, recipient),
                <system::Module<T>>::extrinsic_index(),
                <system::Module<T>>::block_number(),
            ).using_encoded(<T as system::Trait>::Hashing::hash);

            return input;
        }

}

// impl<T: Trait> ContractAddressFor<CodeHash<T>, T::AccountId> for SimpleAddressDeterminator<T>
impl<T: Trait> Posting<T::AccountId,Account> for Module<T> {
    fn get_ledger_balance(who: T::AccountId, ledger: Account) -> Self::AccountBalance {    
        <BalanceByLedger<T>>::get(&who, ledger);
    }

    fn get_detail(who: T::AccountId, ledger: Account) -> Self::AccountBalance {    
        <Detail<T>>::get(&who, ledger);
    }

}

decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    {
        LegderUpdate(AccountId, Account, AccountBalance),
        PrefundingDeposit(AccountId, AccountBalance, Deadline),
        ErrorUpdate(),
    }
);
