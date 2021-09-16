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
// This is the module for locking prefunded amounts into the runtime 
//********************************************************//

// This module functions as a pseudo-escrow module, holding funds for a specified period of time and or for a specific beneficiary.
// In addition to locking funds until a deadline, this module also updates the accounting ledger showing that the assets have moved
// There is no automatic release of funds from the locked state so requires that the either the deadline to have past to allow withdrawal 
// or the intervention of the permitted party to withdraw the funds.

// For the initial use of this prefunding module the intended beneficiary is identified by AccountId. 
// In a later version there may be no intended beneficiary (for example for marketplace transactions)
// and therefore the funds may be locked until a cadidate secures the funds.

// A further scenario is forseen where a dispute resolution method that relies upon an independent validator 
// is required to set the lock-release state. 

use parity_codec::{Encode};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap, ensure};
use runtime_primitives::traits::{Convert, Hash}; // Use with node template only
// use node_primitives::{Convert, Hash}; // Use with full node
use system::{self, ensure_signed};
use rstd::prelude::*;
use support::traits::{
    Currency, 
    LockIdentifier, 
    LockableCurrency, 
    WithdrawReason,
};

// Totem Pallets
use accounting::{ Posting };

// Totem Traits
// use crate::accounting_traits::{ Posting };
use crate::prefunding_traits::{ Encumbrance };

// Totem Trait Types
type AccountOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber,<T as accounting::Trait>::CoinAmount>>::Account;
type AccountBalanceOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber,<T as accounting::Trait>::CoinAmount>>::LedgerBalance;

// Other trait types
type CurrencyBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

// Module Types
pub type UnLocked = bool; // 0=Unlocked(false) 1=Locked(true)
pub type Status = u16; // Generic Status for whatever the HashReference refers to
pub type ComparisonAmounts = u128; // Used for comparisons

pub trait Trait: balances::Trait + system::Trait + timestamp::Trait + accounting::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Currency: Currency<Self::AccountId> + LockableCurrency<Self::AccountId, Moment=Self::BlockNumber>;
    type PrefundingConversions: Convert<AccountBalanceOf<Self>, u128> 
    + Convert<AccountBalanceOf<Self>, CurrencyBalanceOf<Self>> 
    + Convert<CurrencyBalanceOf<Self>, AccountBalanceOf<Self>> 
    + Convert<Vec<u8>, LockIdentifier> 
    + Convert<u64, AccountOf<Self>> 
    + Convert<u64, CurrencyBalanceOf<Self>> 
    + Convert<u64, Self::BlockNumber> 
    + Convert<i128, AccountBalanceOf<Self>> 
    + Convert<u128, AccountBalanceOf<Self>> 
    + Convert<u128, i128> 
    + Convert<AccountBalanceOf<Self>, i128> 
    + Convert<CurrencyBalanceOf<Self>, u128>;
    type Accounting: Posting<Self::AccountId,Self::Hash,Self::BlockNumber,Self::CoinAmount>;
}

decl_storage! {
    trait Store for Module<T: Trait> as PrefundingModule {
        // Funds Storage on Prefunding
        // This storage is intended to signal to a marketplace that the originator is prepared to lockup funds to a deadline.
        // If the sender accepts respondence then the funds are moved to the main prefunding account
        // After deadline sender can withdraw funds
        Prefunding get(prefunding): map T::Hash => Option<(CurrencyBalanceOf<T>, T::BlockNumber)>;
        
        // Says who can take the money after deadline. Includes intended owner (same as origin for market posting)
        // 10, sender can take after deadline (initial state)
        // 11, accepted by recipient. (funds locked, nobody can take) 
        // 01, sender approves (recipient can take, or refund)
        // 00, only the recipient authrises sender to retake funds regardless of deadline.
        PrefundingHashOwner get(prefunding_hash_owner): map T::Hash => Option<(T::AccountId, UnLocked, T::AccountId, UnLocked)>;
        
        // List for convenience
        OwnerPrefundingHashList get(owner_prefunding_hash_list): map T::AccountId => Vec<T::Hash>;
        
        // Reference Hash generic status
        // draft(0),
        // submitted(1),
        // Abandoned or cancelled (50),
        // disputed(100), can be resubmitted, if the current status is < 100 return this state
        // rejected(200), can be resubmitted, if the current status is < 100 return this state
        // accepted(300), can no longer be submitted,
        // invoiced(400), can no longer be accepted, 
        // settled(500), can no longer be invoiced,
        // blocked(999),
        // U16MAX, is quasi-error state
        ReferenceStatus get(reference_status): map T::Hash => Status;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// This function reserves funds from the buyer for a specific vendor account (Closed Order). It is used when an order is created.
        /// Quatity is not relevant 
        /// The prefunded amount remains as an asset of the buyer until the order is accepted
        /// Updates only the accounts of the buyer 
        fn prefund_someone(origin, beneficiary: T::AccountId, amount: u128, deadline: T::BlockNumber, tx_uid: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            // check that the beneficiary is not the sender
            ensure!(who != beneficiary, "Beneficiary must be another account");
            let prefunding_hash: T::Hash = Self::get_pseudo_random_hash(who.clone(), beneficiary.clone());
            Self::prefunding_for(who, beneficiary, amount.into(), deadline, prefunding_hash, tx_uid)?;
            
            Ok(())
        }
        /// Creates a single line simple invoice without taxes, tariffs or commissions
        /// This invoice is associated with a prefunded order - therefore needs to provide the hash reference of the order
        /// Updates the accounting for the vendor and the customer
        fn invoice_prefunded_order(origin, payer: T::AccountId, amount: i128, reference: T::Hash, uid: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            Self::send_simple_invoice(who.clone(), payer.clone(), amount, reference, uid)?;
            Ok(())
        }
        /// Buyer pays a prefunded order. Needs to supply the correct hash reference
        /// Updates bother the buyer and the vendor accounts 
        fn pay_prefunded_invoice(origin, reference: T::Hash, uid: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            Self::settle_prefunded_invoice(who.clone(), reference, uid)?;
            Ok(())
        }
        
        /// Is used by the buyer to recover funds if the vendor does not accept the order by the deadline
        fn cancel_prefunded_closed_order(origin, reference: T::Hash, uid: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            Self::unlock_funds_for_owner(who.clone(), reference, uid)?;
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Reserve the prefunding deposit
    fn set_prefunding(s: T::AccountId, c: AccountBalanceOf<T>, d: T::BlockNumber, h: T::Hash, u: T::Hash) -> Result {
        
        // Prepare make sure we are not taking the deposit again
        if <ReferenceStatus<T>>::exists(&h) {
            Self::deposit_event(RawEvent::ErrorHashExists(u));
            return Err("This hash already exists!");
        }
        
        
        // You cannot prefund any amount unless you have at least at balance of 1618 units + the amount you want to prefund            
        // Ensure that the funds can be subtracted from sender's balance without causing the account to be destroyed by the existential deposit 
        let min_balance: ComparisonAmounts =  1618u128;
        let current_balance: ComparisonAmounts = <T::PrefundingConversions as Convert<CurrencyBalanceOf<T>, u128>>::convert(T::Currency::free_balance(&s));
        let prefund_amount: ComparisonAmounts = <T::PrefundingConversions as Convert<AccountBalanceOf<T>, u128>>::convert(c.clone());
        let minimum_amount: ComparisonAmounts = min_balance + prefund_amount;        
        
        if current_balance >= minimum_amount {
            let converted_amount: CurrencyBalanceOf<T> = <T::PrefundingConversions as Convert<AccountBalanceOf<T>, CurrencyBalanceOf<T>>>::convert(c.clone());
            
            // Lock the amount from the sender and set deadline
            T::Currency::set_lock(Self::get_prefunding_id(h), &s, converted_amount, d, WithdrawReason::Reserve.into());
            
        } else {
            Self::deposit_event(RawEvent::ErrorInsufficientPreFunds(s, prefund_amount, minimum_amount, current_balance));
            return Err("Not enough funds to prefund");
        }
        
        Ok(())
    }
    /// Generate Prefund Id from hash  
    fn get_prefunding_id(hash: T::Hash) -> LockIdentifier {
        // Convert Hash to ID using first 8 bytes of hash
        return <T::PrefundingConversions as Convert<Vec<u8>, LockIdentifier>>::convert(hash.encode());
    }
    /// generate reference hash
    fn get_pseudo_random_hash(sender: T::AccountId, recipient: T::AccountId) -> T::Hash {
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
    /// check hash exists and is valid
    fn reference_valid(h: T::Hash) -> bool {
        match <ReferenceStatus<T>>::get(&h) {
            0 | 1 | 100 | 200 | 300 | 400 => return true,
            _ => return false,
        }
    }
    /// Prefunding deadline passed?
    fn prefund_deadline_passed(h: T::Hash) -> bool {
        let current_block: T::BlockNumber = <system::Module<T>>::block_number();
        match Self::prefunding(&h) {
            Some(deadline) => {
                if Some(deadline.1) <= Some(current_block) { return true } else { () } 
            },
            None => (),
        };
        return false;
    }
    /// Gets the state of the locked funds. The hash needs to be prequalified before passing in as no checks performed here.
    fn get_release_state(h: T::Hash) -> (UnLocked, UnLocked) {
        let owners = Self::prefunding_hash_owner(&h).unwrap();
        return (owners.1, owners.3);
    }
    /// cancel lock for owner
    fn cancel_prefunding_lock(o: T::AccountId, h: T::Hash, s: Status) -> Result {
        // funds can be unlocked for the owner
        // convert hash to lock identifyer
        let prefunding_id = Self::get_prefunding_id(h);
        // unlock the funds
        T::Currency::remove_lock(prefunding_id, &o);
        // perform cleanup removing all reference hashes. No accounting posting have been made, so no cleanup needed there
        <Prefunding<T>>::take(&h);
        <PrefundingHashOwner<T>>::take(&h);
        <ReferenceStatus<T>>::insert(&h, s); // This sets the status but does not remove the hash
        <OwnerPrefundingHashList<T>>::mutate(&o, |owner_prefunding_hash_list| owner_prefunding_hash_list.retain(|e| e != &h));
        // Issue event
        Self::deposit_event(RawEvent::PrefundingCancelled(o, h));
        Ok(())
    }
    /// unlock & pay beneficiary with funds transfer and account updates (settlement of invoice)
    fn unlock_funds_for_beneficiary(o: T::AccountId, h: T::Hash, u: T::Hash) -> Result {
        match Self::reference_valid(h) {
            true => {
                match Self::check_ref_beneficiary(o.clone(), h) { // TODO this should return the details otherwise there is second read later in the process
                    true => {
                        match Self::get_release_state(h) {
                            (true, false)  => { // submitted, but not yet accepted
                                Self::deposit_event(RawEvent::ErrorNotApproved(u));
                                return Err("The demander has not approved the work yet!");
                            },
                            (true, true) => {
                                Self::deposit_event(RawEvent::ErrorFundsInPlay(u));
                                return Err("Funds locked for intended purpose by both parties.")
                            },
                            (false, true) => { 
                                // Owner has approved now get status of hash. Only allow if invoiced.
                                // Note handling the account posting is done outside of this function
                                match <ReferenceStatus<T>>::get(&h) {
                                    400 => {
                                        // get details of lock
                                        let details = Self::prefunding_hash_owner(&h).ok_or("Error fetching details")?;
                                        // get details of prefunding
                                        let prefunding = Self::prefunding(&h).ok_or("Error getting prefunding details")?;
                                        // Cancel prefunding lock
                                        let status:  Status = 500; // Settled
                                        match Self::cancel_prefunding_lock(details.0.clone(), h, status) {
                                            Ok(_) => {
                                                // transfer to beneficiary.
                                                // TODO when currency conversion is implemnted the payment should be at the current rate for the currency
                                                match T::Currency::transfer(&details.0, &o, prefunding.0) {
                                                    Ok(_) => (),
                                                    Err(_) => return Err("Error during transfer"),
                                                }
                                            },
                                            Err(e) => return Err(e),
                                        }
                                        
                                    },
                                    _ => return Err("Only allowed when status is Invoiced"),
                                }
                            },
                            (false, false) => {
                                // Owner has been given permission by beneficiary to release funds
                                Self::deposit_event(RawEvent::ErrorNotAllowed1(u));
                                return Err("Funds already locked for intended purpose by both parties.")
                                
                            },
                        }
                    },
                    false => {
                        Self::deposit_event(RawEvent::ErrorNotOwner(u));
                        return Err("You are not the owner of the hash!");
                    },
                }
            }, 
            false => {
                Self::deposit_event(RawEvent::ErrorHashDoesNotExist(u));
                return Err("Hash does not exist!");
            }, 
        }
        
        Ok(())
    }
    // set the status for the prefunding
    fn set_ref_status(h: T::Hash, s: Status) -> Result {
        <ReferenceStatus<T>>::remove(&h);
        <ReferenceStatus<T>>::insert(&h, s);
        Ok(())
    }
    // TODO Check should be made for available balances, and if the amount submitted is more than the invoice amount. 
    // Settles invoice by updates to various relevant accounts and transfer of funds 
    fn settle_unfunded_invoice() -> Result {
        Ok(())
    }
}

impl<T: Trait> Encumbrance<T::AccountId,T::Hash,T::BlockNumber> for Module<T> {
    
    type UnLocked = UnLocked;
    
    fn prefunding_for(who: T::AccountId, recipient: T::AccountId, amount: u128, deadline: T::BlockNumber, ref_hash: T::Hash, uid: T::Hash) -> Result {
        
        // As amount will always be positive, convert for use in accounting
        let amount_converted: AccountBalanceOf<T> = <T::PrefundingConversions as Convert<u128, AccountBalanceOf<T>>>::convert(amount);  
        // Convert this for the inversion
        let mut to_invert: i128 = <T::PrefundingConversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount_converted.clone());
        // invert the amount
        to_invert = to_invert * -1;
        
        let increase_amount: AccountBalanceOf<T> = amount_converted.clone();
        let decrease_amount: AccountBalanceOf<T> = <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(to_invert);
        
        let current_block = <system::Module<T>>::block_number();
        
        // Prefunding is always recorded in the same block. It cannot be posted to another period
        let current_block_dupe = <system::Module<T>>::block_number(); 
        
        let prefunding_hash: T::Hash = ref_hash.clone();
        
        // convert the account balanace to the currency balance (i128 -> u128)
        let currency_amount: CurrencyBalanceOf<T> = <T::PrefundingConversions as Convert<AccountBalanceOf<T>, CurrencyBalanceOf<T>>>::convert(amount_converted.clone());
        
        // NEED TO CHECK THAT THE DEADLINE IS SENSIBLE!!!!
        // 48 hours is the minimum deadline. This is the minimum amountof time before the money can be reclaimed
        let minimum_deadline: T::BlockNumber = current_block + <T::PrefundingConversions as Convert<u64, T::BlockNumber>>::convert(11520u64);
        
        if deadline < minimum_deadline {
            Self::deposit_event(RawEvent::ErrorShortDeadline(uid));
            return Err("Deadline is too short!");
        }
        
        let prefunded = (currency_amount, deadline.clone());
        
        let owners = (who.clone(), true, recipient.clone(), false);
        
        // manage the deposit
        match Self::set_prefunding(who.clone(), amount_converted.clone(), deadline, prefunding_hash, uid) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorPrefundNotSet(uid));
                return Err("Deposit was not taken");
            },
        };
        // Deposit taken at this point. Note that if an error occurs beyond here we need to remove the locked funds.            
        
        // Buyer
        let account_1: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(110100050000000u64); // Debit  increase 110100050000000 Totem Runtime Deposit (Escrow)
        let account_2: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(110100040000000u64); // credit decrease 110100040000000 XTX Balance
        let account_3: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(360600040000000u64); // Debit increase 360600040000000 Escrowed Funds Control
        
        // Keys for posting
        let mut forward_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(3);
        forward_keys.push((who.clone(), recipient.clone(), account_1, increase_amount, true, prefunding_hash, current_block, current_block_dupe));
        forward_keys.push((who.clone(), recipient.clone(), account_2, decrease_amount, false, prefunding_hash, current_block, current_block_dupe));
        forward_keys.push((who.clone(), recipient.clone(), account_3, increase_amount, true, prefunding_hash, current_block, current_block_dupe));
        
        // Reversal keys in case of errors
        let mut reversal_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(2);
        reversal_keys.push((who.clone(), recipient.clone(), account_1, decrease_amount, false, prefunding_hash, current_block, current_block_dupe));
        reversal_keys.push((who.clone(), recipient.clone(), account_2, increase_amount, true, prefunding_hash, current_block, current_block_dupe));
        
        let track_rev_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(3);
        
        match <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber,T::CoinAmount>>::handle_multiposting_amounts(forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone()) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorInAccounting1(uid));
                return Err("An error occured posting to accounts");
            },
        }
        
        // Record Prefunding ownership and status
        <PrefundingHashOwner<T>>::insert(&prefunding_hash, owners); 
        <Prefunding<T>>::insert(&prefunding_hash, prefunded);
        
        // Add reference hash to list of hashes
        <OwnerPrefundingHashList<T>>::mutate(&who, |owner_prefunding_hash_list| owner_prefunding_hash_list.push(prefunding_hash));
        
        // Submitted, Locked by sender.
        match Self::set_ref_status(prefunding_hash, 1) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorSettingStatus1(uid));
                return Err("Did not set the status");
            },
        }
        
        
        // Issue event
        Self::deposit_event(RawEvent::PrefundingCompleted(uid));
        
        Ok(())
    }
    /// Simple invoice. Does not include tax jurisdiction, tax amounts, freight, commissions, tariffs, discounts and other extended line item values
    /// must include a connection to the originating reference. 
    /// Invoices cannot be made to parties that haven't asked for something identified by a valid hash
    fn send_simple_invoice(o: T::AccountId, p: T::AccountId, n: i128, h: T::Hash, u: T::Hash) -> Result {
        
        // Validate that the hash is indeed assigned to the seller
        match Self::check_ref_beneficiary(o.clone(), h) {
            true => (),
            false => {
                Self::deposit_event(RawEvent::ErrorNotAllowed2(u));
                return Err("Not the beneficiary");
            },
        }
        
        // Amount CAN be negative - this is therefore not an Invoice but a Credit Note!
        // The account postings are identical to an invoice, however we must also handle the refund immediately if possible.
        // In order to proceed with a credit note, validate that the vendor has sufficient funds.
        // If they do not have sufficient funds, the credit note can still be issued, but will remain outstanding until it is settled.
        
        // As amount will always be positive, convert for use in accounting
        let amount_converted: AccountBalanceOf<T> = <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(n.clone());  
        // invert the amount
        let inverted: i128 = n * -1;
        let increase_amount: AccountBalanceOf<T> = amount_converted.clone();
        let decrease_amount: AccountBalanceOf<T> =  <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(inverted);
        
        let current_block = <system::Module<T>>::block_number();
        let current_block_dupe = <system::Module<T>>::block_number();
        
        // Seller
        let account_1: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(110100090000000u64); // Debit increase 110100090000000 Trade receivables - non-related parties
        let account_2: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(240400010000000u64); // Credit increase 240400010000000 Sales of services
        let account_3: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(360600020000000u64); // Debit increase 360600020000000 Sales Control
        
        // Buyer
        let account_4: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(120200030000000u64); // Credit increase 120200030000000 Accounts payable (Trade creditors)
        let account_5: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(250500120000013u64); // Debit  increase 250500120000013	Labour
        let account_6: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(360600010000000u64); // Debit increase 360600010000000 Purchase Control
        
        // Keys for posting
        let mut forward_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(6);
        forward_keys.push((o.clone(), p.clone(), account_1, increase_amount, true, h, current_block, current_block_dupe));
        forward_keys.push((o.clone(), p.clone(), account_2, increase_amount, false, h, current_block, current_block_dupe));
        forward_keys.push((o.clone(), p.clone(), account_3, increase_amount, true, h, current_block, current_block_dupe));
        
        forward_keys.push((p.clone(), o.clone(), account_4, increase_amount, false, h, current_block, current_block_dupe));
        forward_keys.push((p.clone(), o.clone(), account_5, increase_amount, true, h, current_block, current_block_dupe));
        forward_keys.push((p.clone(), o.clone(), account_6, increase_amount, true, h, current_block, current_block_dupe));
        
        // Reversal keys in case of errors
        let mut reversal_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(5);
        reversal_keys.push((o.clone(), p.clone(), account_1, decrease_amount, false, h, current_block, current_block_dupe));
        reversal_keys.push((o.clone(), p.clone(), account_2, decrease_amount, true, h, current_block, current_block_dupe));
        reversal_keys.push((o.clone(), p.clone(), account_3, decrease_amount, false, h, current_block, current_block_dupe));
        
        reversal_keys.push((p.clone(), o.clone(), account_4, decrease_amount, true, h, current_block, current_block_dupe));
        reversal_keys.push((p.clone(), o.clone(), account_5, decrease_amount, false, h, current_block, current_block_dupe));
        
        let track_rev_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(6);
        
        match <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber,T::CoinAmount>>::handle_multiposting_amounts(forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone()) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorInAccounting2(u));
                return Err("There was an error posting to accounts");
            },
        }
        
        // Add status processing
        let new_status: Status = 400; // invoiced(400), can no longer be accepted, 
        
        match Self::set_ref_status(h, new_status) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorSettingStatus2(u));
                return Err("Did not set the status");
            },
        }
        
        // Issue Event
        Self::deposit_event(RawEvent::InvoiceIssued(u));
        Ok(())
    }
    // Settles invoice by unlocking funds and updates various relevant accounts and pays prefunded amount
    fn settle_prefunded_invoice(o: T::AccountId, h: T::Hash, uid: T::Hash) -> Result {
        
        // release state must be 11
        // sender must be owner
        // accounts updated before payment, because if there is an error then the accounting can be rolled back 
        
        let payer: T::AccountId;
        let beneficiary: T::AccountId;
        
        match Self::get_release_state(h) {
            (true, false)  => { // submitted, but not yet accepted
                Self::deposit_event(RawEvent::ErrorNotApproved2(h));
                return Err("The demander has not approved the work yet!");
            },
            (true, true) => {
                
                // Validate that the hash is indeed owned by the buyer
                match Self::check_ref_owner(o.clone(), h) {
                    true => {
                        // get beneficiary from hash
                        // Initialise tuple with dummy values
                        let mut details: (T::AccountId, UnLocked, T::AccountId, UnLocked) = (o.clone(), true, o.clone(), false); 
                        match Self::prefunding_hash_owner(&h) {
                            Some(v) => {
                                details.0 = v.0.clone();
                                details.1 = v.1.clone();
                                details.2 = v.2.clone();
                                details.3 = v.3.clone();
                            },
                            None => {
                                Self::deposit_event(RawEvent::ErrorNoDetails(uid));
                                return Err("Error getting details from hash")
                            },
                        }
                        
                        // get prefunding amount for posting to accounts
                        let temp_balance: CurrencyBalanceOf<T> = <T::PrefundingConversions as Convert<u64, CurrencyBalanceOf<T>>>::convert(0u64);
                        let temp_block: T::BlockNumber = <T::PrefundingConversions as Convert<u64, T::BlockNumber>>::convert(0u64);
                        let mut prefunding: (CurrencyBalanceOf<T>, T::BlockNumber) = (temp_balance, temp_block);
                        match Self::prefunding(&h) {
                            Some(v) => {
                                prefunding.0 = v.0.clone();
                                prefunding.1 = v.1.clone();
                            },
                            None => {
                                Self::deposit_event(RawEvent::ErrorNoPrefunding(uid));
                                return Err("Error getting prefunding details from hash")
                            },
                        }
                        
                        let prefunded_amount: CurrencyBalanceOf<T> = prefunding.0;
                        
                        // convert to Account Balance type
                        let amount: AccountBalanceOf<T> = <T::PrefundingConversions as Convert<CurrencyBalanceOf<T>,AccountBalanceOf<T>>>::convert(prefunded_amount.into());
                        // Convert for calculation
                        let mut to_invert: i128 = <T::PrefundingConversions as Convert<AccountBalanceOf<T>,i128>>::convert(amount.clone());
                        to_invert = to_invert * -1;
                        let increase_amount: AccountBalanceOf<T> = amount;
                        let decrease_amount: AccountBalanceOf<T> = <T::PrefundingConversions as Convert<i128,AccountBalanceOf<T>>>::convert(to_invert);
                        
                        let current_block = <system::Module<T>>::block_number();
                        let current_block_dupe = <system::Module<T>>::block_number();
                        
                        let account_1: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(120200030000000u64); // Debit increase 120200030000000 Accounts payable (Trade creditors)
                        let account_2: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(110100050000000u64); // Credit decrease 110100050000000 Totem Runtime Deposit (Escrow)
                        let account_3: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(360600040000000u64); // Credit decrease 360600040000000 Escrowed Funds Control
                        let account_4: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(360600010000000u64); // Credit decrease 360600010000000 Purchase Control
                        
                        let account_5: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(110100040000000u64); // Debit  increase 110100040000000 XTX Balance
                        let account_6: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(110100090000000u64); // Credit decrease 110100090000000 Trade receivables - non-related parties
                        let account_7: AccountOf<T> = <T::PrefundingConversions as Convert<u64, AccountOf<T>>>::convert(360600020000000u64); // Credit decrease 360600020000000 Sales Control
                        
                        // Keys for posting
                        // Buyer
                        let mut forward_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(7);
                        forward_keys.push((o.clone(), details.2.clone(), account_1, decrease_amount, true, h, current_block, current_block_dupe));           
                        forward_keys.push((o.clone(), details.2.clone(), account_2, decrease_amount, false, h, current_block, current_block_dupe));          
                        forward_keys.push((o.clone(), details.2.clone(), account_3, decrease_amount, false, h, current_block, current_block_dupe));          
                        forward_keys.push((o.clone(), details.2.clone(), account_4, decrease_amount, false, h, current_block, current_block_dupe));          
                        
                        // Seller
                        forward_keys.push((details.2.clone(), o.clone(), account_5, increase_amount, true, h, current_block, current_block_dupe));   
                        forward_keys.push((details.2.clone(), o.clone(), account_6, decrease_amount, false, h, current_block, current_block_dupe));  
                        forward_keys.push((details.2.clone(), o.clone(), account_7, decrease_amount, false, h, current_block, current_block_dupe));  
                        
                        // Reversal keys in case of errors
                        // Buyer
                        let mut reversal_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(6);
                        reversal_keys.push((o.clone(), details.2.clone(), account_1, increase_amount, false, h, current_block, current_block_dupe));
                        reversal_keys.push((o.clone(), details.2.clone(), account_2, increase_amount, true, h, current_block, current_block_dupe));
                        reversal_keys.push((o.clone(), details.2.clone(), account_3, increase_amount, true, h, current_block, current_block_dupe));
                        reversal_keys.push((o.clone(), details.2.clone(), account_4, increase_amount, true, h, current_block, current_block_dupe));
                        
                        // Seller
                        reversal_keys.push((details.2.clone(), o.clone(), account_5, decrease_amount, false, h, current_block, current_block_dupe));
                        reversal_keys.push((details.2.clone(), o.clone(), account_6, increase_amount, true, h, current_block, current_block_dupe));
                        
                        let track_rev_keys = Vec::<(T::AccountId, T::AccountId, AccountOf<T>, AccountBalanceOf<T>, bool, T::Hash, T::BlockNumber, T::BlockNumber)>::with_capacity(7);
                        
                        match <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber,T::CoinAmount>>::handle_multiposting_amounts(forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone()) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInAccounting3(uid));
                                return Err("There was an error posting to accounts");
                            },
                        }
                        
                        // export details for final payment steps
                        payer = o.clone();        
                        beneficiary = details.2.clone();        
                        
                    },
                    false => {
                        Self::deposit_event(RawEvent::ErrorNotAllowed3(uid));
                        return Err("Not the owner");
                    },
                }
                
            },
            (false, true) => { // This state is not allowed for this functions
                Self::deposit_event(RawEvent::ErrorNotAllowed4(uid));
                return Err("This function should not be used for this state")
            },
            (false, false) => {
                // Owner has been given permission by beneficiary to release funds
                Self::deposit_event(RawEvent::ErrorNotAllowed5(uid));
                return Err("Funds locked for intended purpose by both parties.")
                
            },
        }
        
        // Set release lock "buyer who has approved invoice"
        // this may have been set independently, but is required for next step
        match Self::set_release_state(payer.clone(), false, h.clone(), uid.clone()) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorReleaseState(uid));
                return Err("Error setting release state")
            },
        }
        
        // Unlock, tansfer funds and mark hash as settled in full
        match Self::unlock_funds_for_beneficiary(beneficiary.clone(), h.clone(), uid.clone()) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorUnlocking(uid));
                return Err("Error unlocking for beneficiary")
            },
        }
        
        Self::deposit_event(RawEvent::InvoiceSettled(uid));
        Ok(())
    }
    /// check owner (of hash) - if anything fails then returns false
    fn check_ref_owner(o: T::AccountId, h: T::Hash) -> bool {
        let mut answer: bool = false;
        match Self::prefunding_hash_owner(&h) {
            Some(owners) => {
                if Some(owners.0) == Some(o) { 
                    answer = true; 
                } else { 
                    (); 
                } 
            },
            None => (),
        };
        return answer;
    }
    /// Sets the release state by the owner or the beneficiary is only called when something already exists
    fn set_release_state(o: T::AccountId, o_lock: UnLocked, h: T::Hash, uid: T::Hash) -> Result {
        // 0= false, 1=true
        // 10, sender can take after deadline (initial state)
        // 11, accepted by recipient. (funds locked, nobody can take) 
        // 01, sender approves (recipient can take, or refund)
        // 00, only the recipient authorises sender to retake funds regardless of deadline.
        
        // Initialise new tuple with some dummy values
        let mut change: (T::AccountId, UnLocked, T::AccountId, UnLocked) = (o.clone(), false, o.clone(), false);
        
        match Self::prefunding_hash_owner(&h) {
            Some(state_lock) => {
                let locks: (UnLocked, UnLocked) = (state_lock.1, state_lock.3);
                change.0 = state_lock.0.clone();
                change.2 = state_lock.2.clone();
                let commander = state_lock.0.clone();
                let fulfiller = state_lock.2.clone();
                
                match locks {
                    (true,false) => {
                        // In this state the commander has created the lock, but it has not been accepted.
                        // The commander can withdraw the lock (set to false) if the deadline has passed, or 
                        // the fulfiller can accept the order (set to true) 
                        match o_lock {
                            true => {
                                if o == commander {
                                    Self::deposit_event(RawEvent::ErrorWrongState1(uid));
                                    return Err("Error buyer cannot set true");
                                } else if o == fulfiller {
                                    change.1 = state_lock.1;
                                    change.3 = o_lock;
                                } else {
                                    Self::deposit_event(RawEvent::ErrorLockNotAllowed1(uid));
                                    return Err("Error not buyer or seller");
                                };
                            },
                            false => {
                                // We do care if the deadline has passed IF this is the commander calling directly
                                // but that must be handled outside of this function
                                if o == commander {
                                    change.1 = o_lock;
                                    change.3 = state_lock.3;
                                } else if o == fulfiller {
                                    Self::deposit_event(RawEvent::ErrorWrongState2(uid));
                                    return Err("Error fulfiller cannot set false");
                                } else {
                                    Self::deposit_event(RawEvent::ErrorLockNotAllowed2(uid));
                                    return Err("Error not buyer or seller");
                                };
                            },
                        }
                    },
                    (true,true) => {
                        // In this state the commander can change the lock, and they can only change it to false
                        // In this state the fulfiller can change the lock, and they can only change it to false
                        match o_lock {
                            true => {
                                Self::deposit_event(RawEvent::ErrorWrongState3(uid));
                                return Err("Cannot set a lock");
                            },
                            false => {
                                if o == commander {
                                    change.1 = o_lock;
                                    change.3 = state_lock.3;
                                } else if o == fulfiller {
                                    change.1 = state_lock.1;
                                    change.3 = o_lock;
                                } else {
                                    Self::deposit_event(RawEvent::ErrorLockNotAllowed3(uid));
                                    return Err("Error not buyer or seller");
                                };
                            },
                        }
                    },
                    (false,true) => {
                        // In this state the commander cannot change the lock
                        // In this state the fulfiller can change the lock, and they can only change it to false
                        match o_lock {
                            true => {
                                Self::deposit_event(RawEvent::ErrorLockNotAllowed4(uid));
                                return Err("Error not buyer or seller");
                            },
                            false => {
                                if o == commander {
                                    Self::deposit_event(RawEvent::ErrorWrongState5(uid));
                                    return Err("Error seller cannot set false");
                                } else if o == fulfiller {
                                    change.1 = state_lock.1;
                                    change.3 = o_lock;
                                } else {
                                    Self::deposit_event(RawEvent::ErrorLockNotAllowed5(uid));
                                    return Err("Error not buyer or seller");
                                };
                            },
                        }
                        
                    },
                    (false,false) => {
                        // This state should technically make the funds refundable to the buyer. 
                        // Even if the buy wanted to set this state they cannot. Meaning they must create a new order.
                        Self::deposit_event(RawEvent::ErrorLockNotAllowed6(uid));
                        return Err("Error nobody can change this state");
                    },
                }
                
            },
            None => {
                Self::deposit_event(RawEvent::ErrorHashDoesNotExist2(uid));
                return Err("Error getting the hash data");
                
            },
        };
        
        <PrefundingHashOwner<T>>::remove(&h);
        <PrefundingHashOwner<T>>::insert(&h, change);
        
        // Issue event
        Self::deposit_event(RawEvent::PrefundingLockSet(uid));
        
        Ok(())
        
    }
    /// check beneficiary (of hash reference)
    fn check_ref_beneficiary(o: T::AccountId, h: T::Hash) -> bool {
        let mut answer: bool = false;
        match Self::prefunding_hash_owner(&h) {
            Some(owners) => {
                if owners.2 == o { 
                    answer = true;
                } else { 
                    (); 
                } 
            },
            None => (),
        };
        return answer;
    } 
    /// unlock for owner
    fn unlock_funds_for_owner(o: T::AccountId, h: T::Hash, uid: T::Hash) -> Result {
        match Self::reference_valid(h) {
            true => {
                match Self::check_ref_owner(o.clone(), h) {
                    true => {
                        match Self::get_release_state(h) {
                            (true, false)  => { // submitted, but not yet accepted
                                // Check if the dealine has passed. If not funds cannot be release
                                match Self::prefund_deadline_passed(h) {
                                    true => {
                                        let status: Status = 50; // Abandoned or Cancelled
                                        match Self::cancel_prefunding_lock(o.clone(), h, status) {
                                            Ok(_) => (),
                                            Err(_e) => {
                                                Self::deposit_event(RawEvent::ErrorCancelFailed2(uid));
                                                return Err("Cancelling prefunding failed for some reason"); 
                                            },
                                        } 
                                    },
                                    false => { 
                                        Self::deposit_event(RawEvent::ErrorDeadlineInPlay(uid));
                                        return Err("Deadline not yet passed. Wait a bit longer!"); 
                                    },
                                }
                            },
                            (true, true) => {
                                Self::deposit_event(RawEvent::ErrorFundsInPlay2(uid));
                                return Err("Funds locked for intended purpose by both parties.")
                            },
                            (false, true) => {
                                Self::deposit_event(RawEvent::ErrorNotAllowed6(uid));
                                return Err("Funds locked for beneficiary.")
                            },
                            (false, false) => {
                                // Owner has been  given permission by beneficiary to release funds
                                let status:  Status = 50; // Abandoned or cancelled
                                match Self::cancel_prefunding_lock(o.clone(), h, status) {
                                    Ok(_) => (),
                                    Err(_e) => {
                                        Self::deposit_event(RawEvent::ErrorCancellingPrefund(uid));
                                        return Err("Error cancelling prefunding");
                                    }
                                }
                            },
                        }
                    },
                    false => {
                        Self::deposit_event(RawEvent::ErrorNotOwner2(uid));
                        return Err("You are not the owner of the hash!");
                    },
                }
            }, 
            false => {
                Self::deposit_event(RawEvent::ErrorHashDoesNotExist3(uid));
                return Err("Hash does not exist!");
            }, 
        }      
        Ok(())
    }
}

decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    Hash = <T as system::Trait>::Hash,
    ComparisonAmounts = u128,
    {
        PrefundingCancelled(AccountId, Hash),
        PrefundingLockSet(Hash),
        PrefundingCompleted(Hash),
        InvoiceIssued(Hash),
        InvoiceSettled(Hash),
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed1(Hash),
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed2(Hash),
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed3(Hash),
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed4(Hash),
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed5(Hash),
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed6(Hash),
        /// Not enough funds to prefund
        ErrorInsufficientPreFunds(AccountId, ComparisonAmounts, ComparisonAmounts, ComparisonAmounts),
        /// Cannot set this state
        ErrorWrongState1(Hash),
        /// Cannot set this state
        ErrorWrongState2(Hash),
        /// Cannot set this state
        ErrorWrongState3(Hash),
        /// Cannot set this state
        ErrorWrongState4(Hash),
        /// Cannot set this state
        ErrorWrongState5(Hash),
        /// Funds already locked for intended purpose by both parties.
        ErrorNotAllowed1(Hash),
        /// Not the beneficiary
        ErrorNotAllowed2(Hash),
        /// Not the owner
        ErrorNotAllowed3(Hash),
        /// This function should not be used for this state
        ErrorNotAllowed4(Hash),
        /// Funds locked for intended purpose by both parties.
        ErrorNotAllowed5(Hash),
        /// Funds locked for beneficiary.
        ErrorNotAllowed6(Hash),
        /// The demander has not approved the work yet!
        ErrorNotApproved(Hash),
        /// The demander has not approved the work yet!
        ErrorNotApproved2(Hash),
        /// Deadline not yet passed. Wait a bit longer!
        ErrorDeadlineInPlay(Hash),
        /// Funds locked for intended purpose by both parties.
        ErrorFundsInPlay(Hash),
        /// Funds locked for intended purpose by both parties.
        ErrorFundsInPlay2(Hash),
        /// You are not the owner of the hash!
        ErrorNotOwner(Hash),
        /// You are not the owner of the hash!
        ErrorNotOwner2(Hash),
        /// This hash already exists!
        ErrorHashExists(Hash),
        /// Hash does not exist
        ErrorHashDoesNotExist(Hash),
        /// Hash does not exist
        ErrorHashDoesNotExist2(Hash),
        /// Hash does not exist
        ErrorHashDoesNotExist3(Hash),
        /// Deadline is too short! Must be at least 48 hours
        ErrorShortDeadline(Hash),
        /// Deposit was not taken
        ErrorPrefundNotSet(Hash),
        /// An error occured posting to accounts - prefunding for...
        ErrorInAccounting1(Hash),
        /// An error occured posting to accounts - send simple invoice
        ErrorInAccounting2(Hash),
        /// An error occured posting to accounts - settle invoice
        ErrorInAccounting3(Hash),
        /// Did not set the status - prefunding for...
        ErrorSettingStatus1(Hash),
        /// Did not set the status - send simple invoice
        ErrorSettingStatus2(Hash),
        /// Error getting details from hash
        ErrorNoDetails(Hash),
        /// Error setting release state
        ErrorReleaseState(Hash),
        /// Error unlocking for beneficiary
        ErrorUnlocking(Hash),
        /// Error cancelling prefunding
        ErrorCancellingPrefund(Hash),
        /// Error getting prefunding details
        ErrorNoPrefunding(Hash),
        /// Cancelling prefunding failed for some reason
        ErrorCancelFailed(Hash),
        /// Cancelling prefunding failed for some reason
        ErrorCancelFailed2(Hash),
    }
);