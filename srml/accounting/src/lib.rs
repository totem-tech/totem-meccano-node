//!                              Næ§@@@ÑÉ©
//!                        æ@@@@@@@@@@@@@@@@@@
//!                    Ñ@@@@?.?@@@@@@@@@@@@@@@@@@@N
//!                 ¶@@@@@?^%@@.=@@@@@@@@@@@@@@@@@@@@
//!               N@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@
//!               @@@@@@@@?^@@@».............?@@@@@@@@@É
//!              Ñ@@@@@@@@?^@@@@@@@@@@@@@@@@@@'?@@@@@@@@Ñ
//!              @@@@@@@@@?^@@@»..............»@@@@@@@@@@
//!              @@@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@@@
//!              @@@@@@@@@?^ë@@&.@@@@@@@@@@@@@@@@@@@@@@@@
//!               @@@@@@@@?^´@@@o.%@@@@@@@@@@@@@@@@@@@@©
//!                @@@@@@@?.´@@@@@ë.........*.±@@@@@@@æ
//!                 @@@@@@@@?´.I@@@@@@@@@@@@@@.&@@@@@N
//!                  N@@@@@@@@@@ë.*=????????=?@@@@@Ñ
//!                    @@@@@@@@@@@@@@@@@@@@@@@@@@@¶
//!                        É@@@@@@@@@@@@@@@@Ñ¶
//!                             Næ§@@@ÑÉ©

//! Copyright 2020 Chris D'Costa
//! This file is part of Totem Live Accounting.
//! Author Chris D'Costa email: chris.dcosta@totemaccounting.com

//! Totem is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.

//! Totem is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.

//! You should have received a copy of the GNU General Public License
//! along with Totem.  If not, see <http://www.gnu.org/licenses/>.

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

// For example in reporting Amount_ou may drill down to the detail in a heirarchical report like this:
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > CitiCorp Account (Identity)
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > Bank of America Account (Identity)
// Here the Ledger Account has a 1:n relationship to the identities, and therefore aggregates results

// In fact this is just the rearrangement of the attributes or properties of an individual identity
// CitiCorp Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000
// Bank of America Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000
// Here the Identity has a 1:1 relationship to its properties defined in the account number that is being posted to

// Totem Live Accounting Primitives
// * All entities operating on the Totem Live Accounting network have XTX as the Functional Currency. This cannot be changed.
// * All accounting is carried out on Accrual basis.
// * Accounting periods close every block, although entities are free to choose a specific block for longer periods (month/year close is a nominated block number, periods are defined by  block number ranges)
// * In order to facilitate expense recognistion for example the period in which the transaction is recorded, may not necessrily be the period in which the
// transaction is recognised) adjustments must specify the period(block number or block range) to which they relate. By default the transaction block number and the period block number are identical on first posting.

// Curency Types
// The UI provides spot rate for live results for Period close reporting (also known as Reporting Currency or Presentation Currency), which is supported byt the exchange rates module.
// General rules for Currency conversion at Period Close follow GAAP rules and are carried out as follows:
// * Revenue recognition in the period when they occur, and expenses recognised (including asset consumption) in the same period as the revenue to which they relate
// is recognised.
// * All other expenses are recognised in the period in which they occur.
// * Therefore the currency conversion for revenue and related expenses is calculated at the spot rate for the period (block) in which they are recognised.
// * All other currency conversions are made at the rate for the period close. The UI can therefore present the correct conversions for any given value at any point in time.

#![cfg_attr(not(feature = "std"), no_std)]

use parity_codec::{Codec, Decode, Encode};
// use codec::{ Encode, Decode }; // v2

use srml_support::{
    decl_event, decl_module, decl_storage, dispatch::Result, Parameter, StorageMap, StorageValue, ensure,
};
//v1
// use frame_support::{decl_event, decl_error, decl_module, decl_storage, dispatch::DispatchResult, weights::{Weight, DispatchClass}, StorageValue, StorageMap}; // v2

use system::{self, ensure_signed};
//v1
// use frame_system::{self}; //v2

use rstd::prelude::*;
//v1
// use sp_std::prelude::*; //v2

use sr_primitives::traits::{As, Convert, Hash, MaybeSerializeDebug, Member, SimpleArithmetic};
// use sp_runtime::traits::{ Member, Hash }; // v2

use substrate_primitives::crypto::UncheckedFrom;

// totem traits
// use crate::bonsai_traits::{ Storing };

// Balance on an account can be negative
type LedgerBalance = i128;
// General ledger account number
type Account = u64;
// 0=Debit(false) 1=Credit(true) Note: Debit and Credit balances are account specific - see chart of accounts
type Indicator = bool;
// The index number for identifying the posting to ledgers
type PostingIndex = u128;

// Current Accounting Period start and end date. 
#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ClosingDates<BlockNumber> {
    pub period_start: BlockNumber,
    pub period_end: BlockNumber,
}

pub trait Trait: system::Trait + timestamp::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    // The equivalent to Balance trait to avoid cyclical dependency.
    // This is to be used as a replacement for actual network currency
    type CoinAmount: Parameter
    + Member
    + SimpleArithmetic
    + Codec
    + Default
    + Copy
    + As<usize>
    + As<u64>
    + MaybeSerializeDebug;
    
    type AccountingConversions: 
    Convert<Self::CoinAmount, LedgerBalance>
    + Convert<i128, LedgerBalance>
    + Convert<u64, Self::BlockNumber>
    + Convert<LedgerBalance, i128>;
    // type Bonsai: Storing<Self::Hash>;    
}

pub trait Posting<AccountId, Hash, BlockNumber, CoinAmount> {
    type Account: Member + Copy + Eq;
    type PostingIndex: Member + Copy + Into<u128> + Encode + Decode + Eq;
    type LedgerBalance: Member + Copy + Into<i128> + Encode + Decode + Eq;
    fn handle_multiposting_amounts(
        fwd: Vec<(
            AccountId,AccountId,Self::Account,Self::LedgerBalance,bool,Hash,BlockNumber,BlockNumber,
        )>,
        rev: Vec<(
            AccountId,AccountId,Self::Account,Self::LedgerBalance,bool,Hash,BlockNumber,BlockNumber,
        )>,
        trk: Vec<(
            AccountId,AccountId,Self::Account,Self::LedgerBalance,bool,Hash,BlockNumber,BlockNumber,
        )>,
    ) -> Result;
    fn account_for_fees(f: CoinAmount, p: AccountId) -> Result;
    fn get_escrow_account() -> AccountId;
    fn get_netfees_account() -> AccountId;
    fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash;
    fn get_gl_account_balance(sender: AccountId, account: Account) -> LedgerBalance;
    fn force_set_gl_account_balance(sender: AccountId, amount: CoinAmount) -> Result;
}

decl_storage! {
    trait Store for Module<T: Trait> as Accounting {
        // Every accounting post gets an index
        PostingNumber get(posting_number): Option<u128>;
        // Associate the posting index with the identity
        IdAccountPostingIdList get(id_account_posting_id_list): map (T::AccountId, Account) => Vec<u128>;
        // Convenience list of Accounts used by an identity. Useful for UI read performance
        AccountsById get(accounts_by_id): map T::AccountId => Vec<Account>;
        // Accounting Balances
        BalanceByLedger get(balance_by_ledger): map (T::AccountId, Account) => LedgerBalance;
        // Detail of the accounting posting (for Audit)
        PostingDetail get(posting_detail): map (T::AccountId, Account, u128) => Option<(T::AccountId, T::BlockNumber,LedgerBalance,Indicator,T::Hash, T::BlockNumber)>;
        
        // yay! Totem!
        GlobalLedger get(global_ledger): map Account => LedgerBalance;
        // Address to book the sales tax to and the tax jurisdiction (Experimental, may be deprecated in future)
        TaxesByJurisdiction get(taxes_by_jurisdiction): map (T::AccountId, T::AccountId) => LedgerBalance;
        
        // Sets accounting reference date - this is used to determine the start of the current period.
        // Once set it cannot be changed. All annual close periods are calculated from this block onwards
        AccountRefDate get(account_ref_date): map T::AccountId => T::BlockNumber;  
        // Is the current annual accounting period calculated either when the accounting reference date is set or during a close process
        // which is triggered by on finalise. In the cause of the first year (when setting the accounting reference date) the 
        // period could feasibly be longer than one year and up to two years 
        // (Experimental, may be deprecated in favour of on-the-fly calculation later)
        CurrentPeriod get(current_period): map T::AccountId => ClosingDates<T::BlockNumber>;  
        
        // TODO
        // Quantities Accounting
        // Depreciation (calculated everytime there is a transaction so as not to overwork the runtime) - sets "last seen block" to calculate the delta for depreciation
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// This sets the accounting reference date for an AccountId. This can be set only once per AccountId.
        /// Once set no accounting adjustments are permitted before this date.
        /// If accounting entries have been posted before this date, then these are generally considered pre formation costs.
        /// Note in general this should go along with setting opening balances if there are non-blockchain related entries to be made
        /// such as bank account balances, or when migrating from legacy accounting systems.
        /// Arguments:
        /// reference_date = the first blocknumber after midnight for a given date in the past (but no more than 4_204_799 blocks ago)
        /// year_end: no less than 172_799 blocks from now 
        fn set_accounting_ref_date(origin, reference_date: T::BlockNumber, year_end: T::BlockNumber, tx_uid: T::Hash) -> Result {
            // TODO Start Transaction
            // <<T as Trait>::Bonsai as Storing<T::Hash>>::start_tx(tx_uid.clone())?;
            let who = ensure_signed(origin)?;
            // check that the value has not already been set
            ensure!(!<AccountRefDate<T>>::exists(&who), "Accounting reference date is already set!");
            
            // check that the block number is in the past
            let current_block: T::BlockNumber = <system::Module<T>>::block_number();
            if reference_date <= current_block {
                Self::deposit_event(RawEvent::ErrorDateInFuture(tx_uid));
                return Err("Accounting Reference Date is not in the past");
            } 
            
            // check that the block number is not more than two years old (no sense to have a first financial year longer than two years)
            let mut two_years_ago: T::BlockNumber = current_block.clone(); 
            two_years_ago -= <T::AccountingConversions as Convert<u64, T::BlockNumber>>::convert(4_204_800u64);
            
            if reference_date > two_years_ago {
                Self::deposit_event(RawEvent::ErrorDateTooOld(tx_uid));
                return Err("Accounting Reference Date is too far in the past");
            }
            
            // check that the end block number is not less than 30 Days from now (no sense to have first full period close less than a month away)
            let mut thirty_days: T::BlockNumber = <T::AccountingConversions as Convert<u64, T::BlockNumber>>::convert(172_800u64);
            thirty_days += current_block.clone();
            
            if year_end < thirty_days {
                Self::deposit_event(RawEvent::ErrorYearEndTooSoon(tx_uid));
                return Err("Year End is too soon");
            }
            
            let current_period: ClosingDates<T::BlockNumber> = ClosingDates {
                period_start: reference_date.clone(),
                period_end: year_end,
            };
            
            // Set Dates (start / first period end)
            <AccountRefDate<T>>::insert(&who, reference_date);
            <CurrentPeriod<T>>::insert(&who, current_period);
            
            // Issue Event
            Self::deposit_event(RawEvent::ReferenceDatesSet(who,reference_date,year_end));  
            
            // TODO Confirm tyransaction success
            // <<T as Trait>::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;
            
            Ok(())
        }
        /// This function sets the opening balance for an account associated with an Identity.
        /// By default the entry must occur on the accounting reference date, even if entries have been made before that date
        /// TODO checks on the account to see if they are credit or debit balances in Lego.
        /// Meccano cannot automatically do this. Therefore checks are carried out manually
        /// Only positive values (u128) can be received - substraction or addition is performed as a function of the account type and 
        /// the debit or credit indicator 
        fn set_opening_balance(origin, account: Account, account_type: bool, drcr: bool, amount: T::CoinAmount, tx_uid: T::Hash) -> Result {
            // TODO Start Transaction
            // <<T as Trait>::Bonsai as Storing<T::Hash>>::start_tx(tx_uid.clone())?;
            let who = ensure_signed(origin)?;
            
            // Get accounting reference date - reject if it doesn't exist
            ensure!(<AccountRefDate<T>>::exists(&who), "Error accounting reference date not set");
            let accounting_reference_date = <AccountRefDate<T>>::get(&who);
            
            // get current block
            let current_block = <system::Module<T>>::block_number();

            /// This is a dummy counterparty address, but shows the totality of value entered into Totem Accounting (could be useful?)
            // let external_account: [u8;32] = *b"TotalValueComingFromOutsideTotem";
            let external_address: T::AccountId = *b"TotalValueComingFromOutsideTotem";
            // let mut external_address = UncheckedFrom::unchecked_from(external_account);

            // get default hash reference for the opening balance
            let default_ref_hash: T::Hash = Self::get_default_opening_hash();
            
            let mut open_balance: LedgerBalance = <T::AccountingConversions as Convert<i128, LedgerBalance>>::convert(0i128);
            let mut converted_amount: LedgerBalance = <T::AccountingConversions as Convert<T::CoinAmount, LedgerBalance>>::convert(amount);
            
            // TODO in Lego there will be no input argument, it will have to be selected from the account for consistency
            if account_type != drcr {
                // Debit  type & credit entry = decrease value
                // Credit type & debit  entry = decrease value
                open_balance -= converted_amount;
            }
            
            // It is used just for generic self-referential accounting 
            
            // Keys for posting by payer
            let mut forward_keys = Vec::<(
                T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
            )>::with_capacity(1);
            
            forward_keys.push((who.clone(),external_address,account,open_balance,drcr,default_ref_hash,current_block,accounting_reference_date,));
            
            // Reversal keys in case of errors
            let reversal_keys = Vec::<(
                T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
            )>::with_capacity(0);
            
            let track_rev_keys = Vec::<(
                T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
            )>::with_capacity(1);
            
            match Self::handle_multiposting_amounts(forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone()) {
                Ok(_) => (),Err(_e) => {
                    Self::deposit_event(RawEvent::ErrorPostOpenBal());
                    return Err("An error occured posting to accounts");
                },
            }
            // TODO Confirm tyransaction success
            // <<T as Trait>::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;
            Ok(())
        }
        
        /// This function allows accounting adjustments to be made to the accounts.
        /// It does not refer to new postings,bu
        fn adjust_new() -> Result {
            
            Ok(())
        }
        
        fn adjust_existing() -> Result {
            
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    #[allow(dead_code)]
    /// Basic posting function (warning! can cause imbalance if not called with corresponding debit or credit entries)
    /// The reason why this is a simple function is that (for example) one debit posting may correspond with one or many credit
    /// postings and vice-versa. For example a debit to Accounts Receivable is the gross invoice amount, which could correspond with
    /// a credit to liabilities for the sales tax amount and a credit to revenue for the net invoice amount. The sum of both credits being
    /// equal to the single debit in accounts receivable, but only one posting needs to be made to that account, and two posting for the others.
    /// The Totem Accounting Recipes are constructed using this simple function.
    /// The second Blocknumber is for re-targeting the entry in the accounts, i.e. for adjustments prior to or after the current period (generally accruals).
    fn post_amounts(
        (o, p, a, c, d, h, b, t, i): (
            T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,PostingIndex,
        ),
    ) -> Result {
        let new_balance: LedgerBalance;
        let new_global_balance: LedgerBalance;
        let posting_index = i.into();
        
        let ab: LedgerBalance = c.abs();
        let balance_key = (o.clone(), a);
        let posting_key = (o.clone(), a, posting_index);
        let detail = (p, b, ab, d, h, t);
        // !! Warning !!
        // Values could feasibly overflow, with no visibility on other accounts. In this event this function returns an error.
        // Reversals must occur in the parent function (i.e. that calls this function).
        // As all values passed to this function are already signed +/- we only need to sum to the previous balance and check for overflow
        // Updates are only made to storage once tests below are passed for debits or credits.
        match Self::balance_by_ledger(&balance_key).checked_add(c) {
            None => {
                Self::deposit_event(RawEvent::ErrorOverflow(a));
                return Err("Balance Value overflowed");
            }
            Some(l) => {
                new_balance = l;
                match Self::global_ledger(&a).checked_add(c) {
                    Some(g) => new_global_balance = g,        
                    None => {
                        Self::deposit_event(RawEvent::ErrorGlobalOverflow());
                        return Err("Global Balance Value overflowed");
                    }
                }
            }
        };
        
        <PostingNumber<T>>::put(posting_index);
        // The index should be unique, it may already have been posted?
        <IdAccountPostingIdList<T>>::mutate(&balance_key, |id_account_posting_id_list| {id_account_posting_id_list.retain(|i| i != &posting_index)});
        <IdAccountPostingIdList<T>>::mutate(&balance_key, |id_account_posting_id_list| {id_account_posting_id_list.push(posting_index)});
        
        <AccountsById<T>>::mutate(&o, |accounts_by_id| accounts_by_id.retain(|h| h != &a));
        <AccountsById<T>>::mutate(&o, |accounts_by_id| accounts_by_id.push(a));
        // <BalanceByLedger<T>>::remove(&balance_key);
        <BalanceByLedger<T>>::insert(&balance_key, new_balance);
        // <PostingDetail<T>>::remove(&posting_key);
        <PostingDetail<T>>::insert(&posting_key, detail);
        // <GlobalLedger<T>>::remove(&a);
        <GlobalLedger<T>>::insert(&a, new_global_balance);
        
        Self::deposit_event(RawEvent::LegderUpdate(o, a, c, posting_index));
        
        Ok(())
    }
    /// generic default hash for opening balances
    fn get_default_opening_hash() -> T::Hash {
        let default_bytes = "Default opening balance hash";
        let default_hash: T::Hash = T::Hashing::hash(&default_bytes.encode().as_slice());
        return default_hash;
    }
}

impl<T: Trait> Posting<T::AccountId, T::Hash, T::BlockNumber, T::CoinAmount> for Module<T> 
where 
T::AccountId: UncheckedFrom<[u8; 32]>,
{
    type Account = Account;
    type LedgerBalance = LedgerBalance;
    type PostingIndex = PostingIndex;
    
    /// The Totem Accounting Recipes are constructed using this function which handles posting to multiple accounts.
    /// It is exposed to other modules as a trait
    /// If for whatever reason an error occurs during the storage processing which is sequential
    /// this function also handles reversing out the prior accounting entries
    /// Therefore the recipes that are passed as arguments need to be be accompanied with a reversal
    /// Obviously the last posting does not need a reversal for if it errors, then it was not posted in the first place.
    fn handle_multiposting_amounts(
        // o: <T as system::Trait>::AccountId,
        // o: T::AccountId,
        fwd: Vec<(
            T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
        )>,
        rev: Vec<(
            T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
        )>,
        trk: Vec<(
            T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
        )>,
    ) -> Result {
        let reversal_keys = rev.clone();
        let mut track_rev_keys = trk.clone();
        let length_limit = track_rev_keys.len();
        
        let mut posting_index: PostingIndex = 0;
        if <PostingNumber<T>>::exists() {
            posting_index = Self::posting_number().ok_or("Error fetching latest posting index")?;
            match posting_index.checked_add(1) {
                Some(i) => posting_index = i,    
                None => {
                    Self::deposit_event(RawEvent::ErrorGlobalOverflow());
                    return Err("Posting Index Overflowed!");
                }
            }
        }
        
        // Iterate over forward keys. If Ok add reversal key to tracking, if error, then reverse out prior postings.
        for (pos, a) in fwd.clone().iter().enumerate() {
            // build tuple for posting
            let p = (a.0.clone(), a.1.clone(), a.2, a.3, a.4, a.5, a.6, a.7, posting_index);
            
            // match Self::post_amounts(a.clone()) {
                match Self::post_amounts(p.clone()) {
                    Ok(_) => {
                        if pos < length_limit {
                            track_rev_keys.push(reversal_keys[pos].clone())
                        };
                    }
                    Err(_e) => {
                        // Error before the value was updated. Need to reverse-out the earlier debit amount and account combination
                        // as this has already changed in storage.
                        for (_dummy_pos, b) in track_rev_keys.iter().enumerate() {
                            let r = (b.0.clone(), b.1.clone(), b.2, b.3, b.4, b.5, b.6, b.7, posting_index);
                            
                            // match Self::post_amounts(b.clone()) {
                                match Self::post_amounts(r.clone()) {
                                    Ok(_) => (),                
                                    Err(_e) => {
                                        // This event is because there is a major system error in the reversal process
                                        Self::deposit_event(RawEvent::ErrorInError());
                                        return Err("System Failure in Account Posting");
                                    }
                                }
                            }
                            Self::deposit_event(RawEvent::ErrorOverflow(a.2));
                            return Err("Overflow error, amount too big!");
                        }
                    }
                }
                Ok(())
            }
            
            /// This function simply returns the Totem escrow account address
            fn get_escrow_account() -> T::AccountId {
                let escrow_account: [u8;32] = *b"TotemsEscrowAddress4LockingFunds";
                UncheckedFrom::unchecked_from(escrow_account)
            }
            /// This function simply returns the Totem network fees account address
            fn get_netfees_account() -> T::AccountId {
                let netfees_account: [u8;32] = *b"TotemAccountingNetworkFeeAddress";
                UncheckedFrom::unchecked_from(netfees_account)
            }            
            /// This function takes the transaction fee and prepares to account for it in accounting.
            /// This is one of the few functions that will set the ledger accounts to be updated here. Fees
            /// are native to the Substrate Framework, and there may be other use cases.
            fn account_for_fees(fee: T::CoinAmount, payer: T::AccountId) -> Result {
                
                // Take the fee amount and convert for use with accounting. Fee is of type T::Balance which is u128.
                // As amount will always be positive, convert for use in accounting
                let fee_converted: LedgerBalance =
                <T::AccountingConversions as Convert<T::CoinAmount, LedgerBalance>>::convert(fee);
                // Convert this for the inversion
                let mut to_invert: LedgerBalance = <T::AccountingConversions as Convert<i128, LedgerBalance>>::convert(0i128);
                to_invert -= fee_converted.clone();
                // to_invert = to_invert * -1;
                let increase_amount: LedgerBalance = fee_converted.into();
                let decrease_amount: LedgerBalance = to_invert.into();
                
                // Sender
                let account_1: Account = 250500300000000u64; // debit  increase 250500300000000 Totem Transaction Fees
                let account_2: Account = 110100040000000u64; // credit decrease 110100040000000 XTX Balance
                
                // Treasury ()
                // let account_2: Account = 240400010000000u64; // debit  increase 110100040000000 XTX Balance
                let account_3: Account = 240400010000000u64; // credit increase 240400010000000 Sales of services
                
                // This sets the change block and the applicable posting period. For this context they will always be
                // the same.
                let current_block = <system::Module<T>>::block_number(); // For audit on change
                let current_block_dupe = current_block.clone(); // Applicable period for accounting
                
                // Generate dummy Hash reference (it has no real bearing but allows posting to happen)
                let fee_hash: T::Hash = Self::get_pseudo_random_hash(payer.clone(), payer.clone());
                
                // Get the dummy address for fees. Note this does not identify the receipients of fees (validators)
                // It is used just for generic self-referential accounting 
                let fee_address: T::AccountId = Self::get_netfees_account();
                
                // Keys for posting by payer
                let mut forward_keys = Vec::<(
                    T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
                )>::with_capacity(4);
                
                // Sender Identity
                forward_keys.push((payer.clone(),fee_address.clone(),account_1,increase_amount,false,fee_hash,current_block,current_block_dupe,));
                forward_keys.push((payer.clone(),fee_address.clone(),account_2,decrease_amount,true,fee_hash,current_block,current_block_dupe,));
                
                // Treasury
                forward_keys.push((fee_address.clone(),payer.clone(),account_3,increase_amount,true,fee_hash,current_block,current_block_dupe,));
                forward_keys.push((fee_address.clone(),payer.clone(),account_2,increase_amount,false,fee_hash,current_block,current_block_dupe,));
                
                // Reversal keys in case of errors
                let mut reversal_keys = Vec::<(
                    T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
                )>::with_capacity(3);
                reversal_keys.push((payer.clone(),fee_address.clone(),account_1,decrease_amount,true,fee_hash,current_block,current_block_dupe,));
                // reversal_keys.push((payer.clone(),fee_address.clone(),account_2,increase_amount,false,fee_hash,current_block,current_block_dupe,));
                
                reversal_keys.push((fee_address.clone(),payer.clone(),account_3,decrease_amount,false,fee_hash,current_block,current_block_dupe,));
                // reversal_keys.push((fee_address.clone(),payer.clone(),account_2,decrease_amount,true,fee_hash,current_block,current_block_dupe,));
                
                let track_rev_keys = Vec::<(
                    T::AccountId,T::AccountId,Account,LedgerBalance,bool,T::Hash,T::BlockNumber,T::BlockNumber,
                )>::with_capacity(4);
                
                match Self::handle_multiposting_amounts(forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone()) {
                    Ok(_) => (),Err(_e) => {
                        Self::deposit_event(RawEvent::ErrorPostingFees());
                        return Err("An error occured posting to accounts");
                    },
                }
                
                Ok(())
            }
            
            fn get_pseudo_random_hash(sender: T::AccountId, recipient: T::AccountId) -> T::Hash {
                let tuple = (sender, recipient);
                let input = (
                    tuple,<timestamp::Module<T>>::get(),<system::Module<T>>::random_seed(),<system::Module<T>>::extrinsic_index(),<system::Module<T>>::block_number(),
                );
                return T::Hashing::hash(input.encode().as_slice()); // default hash BlakeTwo256
            }
            
            fn get_gl_account_balance(sender: T::AccountId, account: Account) -> LedgerBalance {
                let key = (sender, account);
                let mut balance: LedgerBalance = 0;
                if <BalanceByLedger<T>>::exists(&key) {
                    balance = Self::balance_by_ledger(&key);
                }
                return balance;
            }
            // DO NOT MIGRATE TO LEGO - This Function only exists for Meccano Network 
            fn force_set_gl_account_balance(account_id: T::AccountId, amount: T::CoinAmount) -> Result {
                let account: Account = 110100040000000u64;
                let key = (account_id, account);
                let amount_converted: LedgerBalance =
                <T::AccountingConversions as Convert<T::CoinAmount, LedgerBalance>>::convert(amount);
                <BalanceByLedger<T>>::insert(key, amount_converted);
                <GlobalLedger<T>>::remove(account);
                Ok(())
            }
            // ^^^^^^^^^^^^ DO NOT MIGRATE TO LEGO - This Function only exists for Meccano Network 
        }
        
        decl_event!(
            pub enum Event<T>
            where
            AccountId = <T as system::Trait>::AccountId,
            Account = u64,
            LedgerBalance = i128,
            PostingIndex = u128,
            Hash = <T as system::Trait>::Hash,
            Block = <T as system::Trait>::BlockNumber,
            {
                LegderUpdate(AccountId, Account, LedgerBalance, PostingIndex),
                ReferenceDatesSet(AccountId, Block, Block),
                ErrorOverflow(Account),
                ErrorGlobalOverflow(),
                ErrorInError(),
                ErrorPostingFees(),
                ErrorBalanceAlignment(),
                ErrorDateInFuture(Hash),
                ErrorDateTooOld(Hash),
                ErrorYearEndTooSoon(Hash),
                ErrorPostOpenBal(),
            }
        );
        