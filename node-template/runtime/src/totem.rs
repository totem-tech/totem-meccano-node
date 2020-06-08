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

// For example in reporting yAmount_ou may drill down to the detail in a heirarchical report like this:
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > CitiCorp Account (Identity)
// 110100010000000 Balance Sheet > Assets > Current Assets > Bank Current > Bank of America Account (Identity)
// Here the Ledger Account has a 1:n relationship to the identities, and therefore aggregates results

// But actually this is just the rearrangement of the attributes (or properties of an individual identity 
    // CitiCorp Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000 
    // Bank of America Account (Identity) has properties > Bank Current > Current Assets > Assets > Balance Sheet > 110100010000000 
    // Here the Identity has a 1:1 relationship to its properties
    
    // It also contains a generic prefunding module.
    
    //********************************************************//
    use parity_codec::{Decode, Encode};
    use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap, ensure};
    use runtime_primitives::traits::{Convert, Hash}; // Use with node template only
    // use node_primitives::{Zero, Hash}; // Use with full node
    use system::{self, ensure_signed};
    use rstd::prelude::*;
    use support::traits::{
        Currency, 
        // OnFreeBalanceZero, 
        // OnDilution, 
        LockIdentifier, 
        LockableCurrency, 
        // ReservableCurrency, 
        // WithdrawReasons, 
        WithdrawReason,
        // OnUnbalanced, 
        // Imbalance,
    };
    
    type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
    // type PositiveImbalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::PositiveImbalance;
    // type NegativeImbalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::NegativeImbalance;
    
    pub type AccountBalance = i128; // Balance on an account can be negative - needs to be larger than the 
    pub type Account = u64; // General ledger account number
    pub type Indicator = bool; // 0=Debit(false) 1=Credit(true) Note: Debit and Credit balances are account specific - see chart of accounts
    pub type UnLocked = bool; // 0=Unlocked(false) 1=Locked(true)
    // pub type Symbol = Vec<u8>; // Currency Symbol 
    // pub type Rate = u32; // Exchange Rate
    pub type Status = u16; // Generic Status for whatever the HashReference refers to
    // pub type Moment = u64;
    
    pub const U16MAX: u16 = u16::max_value();
    
    pub trait Trait: balances::Trait + system::Trait + timestamp::Trait {
        type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
        type Currency: Currency<Self::AccountId> + LockableCurrency<Self::AccountId, Moment=Self::BlockNumber>;
        type Conversions: Convert<AccountBalance, BalanceOf<Self>> + Convert<BalanceOf<Self>, AccountBalance> + Convert<Vec<u8>, LockIdentifier>;
    }
    
    decl_storage! {
        trait Store for Module<T: Trait> as TotemModule {
            // Accounting Balances 
            BalanceByLedger get(balance_by_ledger): map (T::AccountId, Account) => AccountBalance;
            // Detail of the accounting posting (for auditAmount_)
            AmountDetail get(detail): map (T::AccountId, Account) => Option<(T::BlockNumber,AccountBalance,Indicator,T::Hash)>;
            // Address to book the sales tax to, and the tax jurisdiction 
            SalesTax get(sales_tax): map (T::AccountId, T::AccountId) => AccountBalance;
            
            // yay! Totem!
            GlobalLedger get(global_ledger): map Account => AccountBalance;
            
            // Funds Storage on Prefunding
            // This storage is intended to signal to a marketplace that the originator is prepared to lockup funds to a deadline.
            // If the sender accepts respondence then the funds are moved to the main prefunding account
            // After deadline sender can withdraw funds
            Prefunding get(prefunding): map T::Hash => Option<(BalanceOf<T>, T::BlockNumber)>;
            
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
            fn prefund_closed_order(origin, beneficiary: T::AccountId, amount: AccountBalance, deadline: T::BlockNumber) -> Result {
                let who = ensure_signed(origin)?;
                // check that the beneficiary is not the sender
                ensure!(who != beneficiary, "Beneficiary must be another account");
                let _ = Self::prefunding_for(who, beneficiary, amount, deadline)?;
                
                Ok(())
            }
            /// Creates a single line simple invoice without taxes, tariffs or commissions
            /// This invoice is associated with a prefunded order - therefore needs to provide the hash reference of the order
            /// Updates the accountiung for the vendor and the customer
            fn invoice_prefunded_order(origin, payer: T::AccountId, amount: AccountBalance, reference: T::Hash) -> Result {
                let who = ensure_signed(origin)?;
                let _ = Self::send_simple_invoice(who.clone(), payer.clone(), amount, reference)?;
                Ok(())
            }
            /// Buyer pays a prefunded order. Needs to supply the correct hash reference
            /// Updates bother the buyer and the vendor accounts 
            fn pay_prefunded_invoice(origin, reference: T::Hash) -> Result {
                let who = ensure_signed(origin)?;
                let _ = Self::settle_prefunded_invoice(who.clone(), reference)?;
                Ok(())
            }
            /// Setting the prefunded release state effectively locks the funds when the vendor agrees to work
            /// It is generally only changed by the vendor, once the prefund is created            
            fn set_prefund_release_state(origin, lock: UnLocked, reference: T::Hash) -> Result {
                let who = ensure_signed(origin)?;
                // check if they are the beneficary or the buyer/payer
                // a similar check exists in the function, because this could be called by another module
                // that is either vendor or buyer specific, and can therefore "self identify".
                if Self::check_ref_owner(who.clone(), reference) {
                    Self::set_release_state(who.clone(), lock, reference, true)?;
                } else if Self::check_ref_beneficiary(who.clone(), reference) {
                    Self::set_release_state(who.clone(), lock, reference, false)?;
                } else {
                    // Issue error 
                    Self::deposit_event(RawEvent::ErrorLockNotAllowed(reference));
                    return Err("You are not the owner or the beneficiary");
                    
                }
                
                Ok(())
            }
            /// Is ised by the buyer to recover funds if the vendor does not accept the order by the deadline
            fn cancel_prefunded_closed_order(origin, reference: T::Hash) -> Result {
                let who = ensure_signed(origin)?;
                Self::unlock_funds_for_owner(who.clone(), reference)?;
                Ok(())
            }
        }
    }
    
    impl<T: Trait> Module<T> {
        // ****************************************************//
        /// Basic posting function (warning! can cause imbalance if not called with corresponding debit or credit entries)
        /// The reason why this is a simple function is that (for example) one debit posting may correspond with one or many credit
        /// postings and vice-versa. For example a debit to Accounts Receivable is the gross invoice amount, which could correspond with 
        /// a credit to liabilities for the sales tax amount and a credit to revenue for the net invoice amount. The sum of both credits being 
        /// equal to the single debit in accounts receivable, but only one posting needs to be made to that account, and two posting for the others.
        /// The Totem Accounting Recipes are constructed using this simple function.
        fn post_amounts((o, a, c, d, h, b): (T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)) -> Result {
            let zero: AccountBalance = 0;
            let key = (o.clone(), a);
            let abs: AccountBalance = c.abs();
            let detail = (b, abs, d, h);
            
            // !! Warning !! 
            // Values could feasibly overflow, with no visibility on other accounts. Therefore need to handle the error states
            // Reversals must occur in the function that calls this function as updates are made to storage once all three tests are passed for either the debit or credit.
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
                    Some(n) => {
                        match Self::global_ledger(&a).checked_sub(c) {
                            Some(n) => (),
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
        
        fn handle_multiposting_amounts(
            o: T::AccountId, 
            fwd: Vec<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>, 
            rev: Vec<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>, 
            trk: Vec<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>) -> Result {
    
                // let forward_keys = fwd.clone();
                let reversal_keys = rev.clone();
                let mut track_rev_keys = trk.clone();
    
    
                let length_limit = reversal_keys.len();
                // Iterate over forward keys. If Ok add reversal key to tracking, if error, then reverse out prior postings.
                // for (pos, a) in forward_keys.iter().enumerate() {
                for (pos, a) in fwd.clone().iter().enumerate() {
                    
                    match Self::post_amounts(a.clone()) {
                        Ok(_) => { 
                            if pos < length_limit { track_rev_keys.push(reversal_keys[pos].clone()) };
                        },
                        Err(e) => {
                            // Error before the value was updated. Need to reverse-out the earlier debit amount and account combination
                            // as this has already changed in storage.
                            for (pos2, b) in track_rev_keys.iter().enumerate() {
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
        // ****************************************************//
        
        // ****************************************************//
        // Main Prefunding Recipe including Accounting Postings
        fn prefunding_for(who: T::AccountId, recipient: T::AccountId, amount: AccountBalance, deadline: T::BlockNumber) -> Result {
            
            // amount cannot be negative
            let increase_amount: AccountBalance = amount.abs();
            let decrease_amount: AccountBalance = -amount.abs();
            
            let current_block = <system::Module<T>>::block_number();
            let prefunding_hash: T::Hash = Self::get_pseudo_random_value(who.clone(), recipient.clone());
            
            let converted_amount: BalanceOf<T> = <T::Conversions as Convert<AccountBalance, BalanceOf<T>>>::convert(amount);
            let prefunded = (converted_amount, deadline);
            let new_status: Status = 1; // Submitted, Locked by sender.
            let owners = (who.clone(), true, recipient.clone(), false);
            
            // manage the deposit
            match Self::set_prefunding(who.clone(), amount, deadline, prefunding_hash) {
                Ok(_) => (),
                Err(e) => return Err(e),
            }
            
            // Deposit taken at this point. Note that if an error occurs beyond here we need to remove the locked funds.            
            
            // Buyer
            let account_1: Account = 110100050000000; // debit  increase 110100050000000 Prefunding Account
            let account_2: Account = 110100040000000; // credit decrease 110100040000000 XTX Balance
            let account_3: Account = 360600020000000; // debit  increase 360600020000000 Runtime Ledger by Module
            let account_4: Account = 360600060000000; // debit  increase 360600060000000 Runtime Ledger Control
            
            // Keys for posting
            let mut forward_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(10);
            forward_keys.push((recipient.clone(), account_1, increase_amount, true, prefunding_hash, current_block));
            forward_keys.push((recipient.clone(), account_2, decrease_amount, false, prefunding_hash, current_block));
            forward_keys.push((recipient.clone(), account_3, increase_amount, true, prefunding_hash, current_block));
            forward_keys.push((recipient.clone(), account_4, increase_amount, true, prefunding_hash, current_block));
            
            // Reversal keys in case of errors
            let mut reversal_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(9);
            reversal_keys.push((recipient.clone(), account_1, decrease_amount, false, prefunding_hash, current_block));
            reversal_keys.push((recipient.clone(), account_2, increase_amount, true, prefunding_hash, current_block));
            reversal_keys.push((recipient.clone(), account_3, decrease_amount, false, prefunding_hash, current_block));
            reversal_keys.push((recipient.clone(), account_4, decrease_amount, false, prefunding_hash, current_block));
            
            let track_rev_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(9);
            
            let _ = Self::handle_multiposting_amounts(who.clone(),forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone())?;
            
            // Record Prefunding ownership and status
            <PrefundingHashOwner<T>>::insert(&prefunding_hash, owners); 
            <Prefunding<T>>::insert(&prefunding_hash, prefunded);
            
            // Add reference hash to list of hashes
            <OwnerPrefundingHashList<T>>::mutate(&who, |owner_prefunding_hash_list| {
                owner_prefunding_hash_list.push(prefunding_hash)
            });
            
            <ReferenceStatus<T>>::insert(&prefunding_hash, new_status);
            
            // Issue event
            Self::deposit_event(RawEvent::PrefundingCompleted(who));
            
            Ok(())
        }
        // Reserve the prefunding deposit
        fn set_prefunding(s: T::AccountId, c: AccountBalance, d: T::BlockNumber, h: T::Hash) -> Result {
            
            // Prepare make sure we are not taking the deposit again
            ensure!(!<ReferenceStatus<T>>::exists(&h), "This hash already exists!");        
            
            // You cannot prefund any amount unless you have at least at balance of 1618 units + the amount you want to prefund.
            ensure!((c > 0), "Cannot prefund zero");
            let converted_amount: BalanceOf<T> = <T::Conversions as Convert<AccountBalance, BalanceOf<T>>>::convert(c);
            let minimum_balance: BalanceOf<T> = <T::Conversions as Convert<AccountBalance, BalanceOf<T>>>::convert(1618) + converted_amount;        
            let current_balance = T::Currency::free_balance(&s);
            
            // Ensure that the funds can be substrated from sender's balance 
            // without causing the account to be destroyed by the existential deposit 
            if current_balance >= minimum_balance {
                
                // Lock the amount from the sender and set deadline
                T::Currency::set_lock(Self::get_prefunding_id(h), &s, converted_amount, d, WithdrawReason::Reserve.into());
                
                Self::deposit_event(RawEvent::PrefundingDeposit(s, c, d));
                
                Ok(())
                
            } else {
                Self::deposit_event(RawEvent::ErrorInsufficientFunds(s));
                return Err("Not enough funds to prefund");
            }
        }
        // ****************************************************//
        
        // ****************************************************//
        // Utility functions
        // Generate Prefund Id from hash  
        fn get_prefunding_id(hash: T::Hash) -> LockIdentifier {
            // Convert Hash to ID using first 8 bytes of hash
            return <T::Conversions as Convert<Vec<u8>, LockIdentifier>>::convert(hash.encode());
        }
        // generate reference hash
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
        // Get status of a reference hash or Max Value as error (will need to be checked outside this call) 
        fn get_ref_status(h: T::Hash) -> Status {
            match Self::reference_status(&h) {
                status => status,
                _ => U16MAX, // just return the max value as quasi-error state
            }
        }
        // check owner (of hash) - if anything fails then 
        fn check_ref_owner(o: T::AccountId, h: T::Hash) -> bool {
            match Self::prefunding_hash_owner(&h) {
                Some(owners) => {
                    if Some(owners.0) == Some(o) { return true } else { () } 
                },
                None => (),
            };
            return false;
        }
        // check beneficiary (of hash reference)
        fn check_ref_beneficiary(o: T::AccountId, h: T::Hash) -> bool {
            match Self::prefunding_hash_owner(&h) {
                Some(owners) => {
                    if Some(owners.2) == Some(o) { return true } else { () } 
                },
                None => (),
            };
            return false;
        }    
        // check hash exists and is valid
        fn reference_exists(h: T::Hash) -> bool {
            match <ReferenceStatus<T>>::get(&h) {
                0 | 1 | 100 | 200 | 300 | 400 => return true,
                _ => return false,
            }
        }
        // Prefunding deadline passed?
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
        // Sets the release state by the owner or the beneficiary
        fn set_release_state(o: T::AccountId, o_lock: UnLocked, h: T::Hash, sender: bool) -> Result {
            // 0= false, 1=true
            // 10, sender can take after deadline (initial state)
            // 11, accepted by recipient. (funds locked, nobody can take) 
            // 01, sender approves (recipient can take, or refund)
            // 00, only the recipient authorises sender to retake funds regardless of deadline.
            
            let change: (T::AccountId, UnLocked, T::AccountId, UnLocked);     
            if sender {
                match Self::check_ref_owner(o.clone(), h) {
                    true => {
                        match Self::prefunding_hash_owner(&h) {
                            Some(owners) => {
                                if owners.1 != o_lock && owners.1 == false {
                                    change = (o.clone(), o_lock, owners.2, owners.3);
                                } else {
                                    Self::deposit_event(RawEvent::ErrorLockNotAllowed(h));
                                    return Err("Owner does not have permission to change this value");
                                }
                            },
                            None => return Err("Error getting the hash data"),
                        }
                    },
                    false => {
                        Self::deposit_event(RawEvent::ErrorLockNotAllowed(h));
                        return Err("Not the owner, cannot change lock");
                    },
                };
            } else {
                match Self::check_ref_beneficiary(o.clone(), h) {
                    true => {
                        match Self::prefunding_hash_owner(&h) {
                            Some(owners) => change = (owners.0, owners.1, o.clone(), o_lock),
                            None => return Err("Error getting the hash data"),
                        }
                    },
                    false => {
                        Self::deposit_event(RawEvent::ErrorLockNotAllowed(h));
                        return Err("Not the beneficiary, cannot change lock");
                    },
                }
            }
            
            <PrefundingHashOwner<T>>::remove(&h);
            <PrefundingHashOwner<T>>::insert(&h, change);

            // Issue event
            Self::deposit_event(RawEvent::PrefundingLockSet(o, h));
            
            Ok(())
            
        }
        // Gets the state of the locked funds. The hash needs to be prequalified before passing in as no checks performed here.
        fn get_release_state(h: T::Hash) -> (UnLocked, UnLocked) {
            let owners = Self::prefunding_hash_owner(&h).unwrap();
            return (owners.1, owners.3);
        }
        // cancel lock for owner
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
        // unlock for owner
        fn unlock_funds_for_owner(o: T::AccountId, h: T::Hash) -> Result {
            match Self::reference_exists(h) {
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
                                                Err(e) => return Err(e),
                                            } 
                                        },
                                        false => { 
                                            Self::deposit_event(RawEvent::ErrorDeadlineInPlay(o, h));
                                            return Err("Deadline not yet passed. Wait a bit longer!"); 
                                        },
                                    }
                                },
                                (true, true) => {
                                    Self::deposit_event(RawEvent::ErrorFundsInPlay(o));
                                    return Err("Funds locked for intended purpose by both parties.")
                                },
                                (false, true) => {
                                    Self::deposit_event(RawEvent::ErrorNotAllowed(h));
                                    return Err("Funds locked for beneficiary.")
                                },
                                (false, false) => {
                                    // Owner has been  given permission by beneficiary to release funds
                                    let status:  Status = 50; // Abandoned or cancelled
                                    match Self::cancel_prefunding_lock(o.clone(), h, status) {
                                        Ok(_) => (),
                                        Err(e) => return Err(e),
                                    }
                                },
                                _ => {
                                    Self::deposit_event(RawEvent::ErrorReleaseState(h));
                                    return Err("Error fetching release state");
                                },
                            }
                        },
                        false => {
                            Self::deposit_event(RawEvent::ErrorNotOwner(o, h));
                            return Err("You are not the owner of the hash!");
                        },
                    }
                }, 
                false => {
                    Self::deposit_event(RawEvent::ErrorHashDoesNotExist(h));
                    return Err("Hash does not exist!");
                }, 
            }      
            Ok(())
        }
        // unlock & pay beneficiary with funds transfer and account updates (settlement of invoice)
        fn unlock_funds_for_beneficiary(o: T::AccountId, h: T::Hash) -> Result {
            match Self::reference_exists(h) {
                true => {
                    match Self::check_ref_beneficiary(o.clone(), h) { // TODO this should return the details otherwise there is second read later in the process
                        true => {
                            match Self::get_release_state(h) {
                                (true, false)  => { // submitted, but not yet accepted
                                    Self::deposit_event(RawEvent::ErrorNotApproved(h));
                                    return Err("The demander has not approved the work yet!");
                                },
                                (true, true) => {
                                    Self::deposit_event(RawEvent::ErrorFundsInPlay(o));
                                    return Err("Funds locked for intended purpose by both parties.")
                                },
                                (false, true) => { // Owner has approved you can withdraw funds
                                    // Note handling the account posting is done outside of this function
                                    // Get status. Only allow if invoiced this will set the status to settled
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
                                                    match T::Currency::transfer(&details.0, &o, prefunding.0) {
                                                        Ok(_) => (),
                                                        Err(e) => return Err("Error during transfer"),
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
                                    Self::deposit_event(RawEvent::ErrorNotAllowed(h));
                                    return Err("Funds locked for intended purpose by both parties.")
                                    
                                },
                                _ => {
                                    Self::deposit_event(RawEvent::ErrorReleaseState(h));
                                    return Err("Error fetching release state");
                                },
                            }
                        },
                        false => {
                            Self::deposit_event(RawEvent::ErrorNotOwner(o, h));
                            return Err("You are not the owner of the hash!");
                        },
                    }
                }, 
                false => {
                    Self::deposit_event(RawEvent::ErrorHashDoesNotExist(h));
                    return Err("Hash does not exist!");
                }, 
            }      
            Ok(())
        }
        // ****************************************************//
        
        // Simple invoice. Does not include tax jurisdiction, tax amounts, freight, commissions, tariffs, discounts and other extended line item values
        // must include a connection to the originating reference. Invoices cannot be made to parties that haven't asked for something.
        fn send_simple_invoice(o: T::AccountId, p: T::AccountId, n: AccountBalance, h: T::Hash) -> Result {
            
            // Validate that the hash is indeed assigned to the seller
            match Self::check_ref_beneficiary(o.clone(), h) {
                true => (),
                false => {
                    Self::deposit_event(RawEvent::ErrorNotAllowed(h));
                    return Err("Not the beneficiary");
                },
            }
            
            // amount cannot be negative
            let increase_amount: AccountBalance = n.abs();
            let decrease_amount: AccountBalance = -n.abs();
            
            let current_block = <system::Module<T>>::block_number();
            
            // Seller
            let account_1: Account = 110100080000000; // Debit  increase 110100080000000	Accounts receivable (Sales Control Account or Trade Debtor's Account)
            let account_2: Account = 240400010000000; // Credit increase 240400010000000	Product or Service Sales
            let account_3: Account = 360600010000000; // Debit  increase 360600010000000	Sales Ledger by Payer
            let account_4: Account = 360600050000000; // Debit  increase 360600050000000	Sales Ledger Control
            
            // Buyer
            let account_5: Account = 120200030000000; // Credit increase 120200030000000	Accounts payable
            let account_6: Account = 250500120000013; // Debit  increase 250500120000013	Labour
            let account_7: Account = 360600030000000; // Debit  increase 360600030000000	Purchase Ledger by Vendor
            let account_8: Account = 360600070000000; // Debit  increase 360600070000000	Purchase Ledger Control       

            // Keys for posting
            let mut forward_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(10);
            forward_keys.push((o.clone(), account_1, increase_amount, true, h, current_block));
            forward_keys.push((o.clone(), account_2, increase_amount, false, h, current_block));
            forward_keys.push((o.clone(), account_3, increase_amount, true, h, current_block));
            forward_keys.push((o.clone(), account_4, increase_amount, true, h, current_block));

            forward_keys.push((p.clone(), account_5, increase_amount, false, h, current_block));
            forward_keys.push((p.clone(), account_6, increase_amount, true, h, current_block));
            forward_keys.push((p.clone(), account_7, increase_amount, true, h, current_block));
            forward_keys.push((p.clone(), account_8, increase_amount, true, h, current_block));

            // Reversal keys in case of errors
            let mut reversal_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(9);
            reversal_keys.push((o.clone(), account_1, decrease_amount, false, h, current_block));
            reversal_keys.push((o.clone(), account_2, decrease_amount, true, h, current_block));
            reversal_keys.push((o.clone(), account_3, decrease_amount, false, h, current_block));
            reversal_keys.push((o.clone(), account_4, decrease_amount, false, h, current_block));

            reversal_keys.push((p.clone(), account_5, decrease_amount, true, h, current_block));
            reversal_keys.push((p.clone(), account_6, decrease_amount, false, h, current_block));
            reversal_keys.push((p.clone(), account_7, decrease_amount, false, h, current_block));
            
            let track_rev_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(9);

            let _ = Self::handle_multiposting_amounts(o.clone(),forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone())?;
            
            // Add status processing
            let new_status: Status = 400; // invoiced(400), can no longer be accepted, 
            <ReferenceStatus<T>>::insert(&h, new_status);
            
            // Issue Event
            Self::deposit_event(RawEvent::InvoiceIssued(h));
            Ok(())
        }
        
        fn set_ref_status() -> Result {
            
            Ok(())
        }

        
        // Settles invoice by unlocking funds and updates various relevant accounts and pays prefunded amount
        fn settle_prefunded_invoice(o: T::AccountId, h: T::Hash) -> Result {
            
            // release state must be 11
            // sender must be owner
            // accounts updated before payment, because if there is an error then the accounting can be rolled back 
            // set settled status
            
            match Self::get_release_state(h) {
                (true, false)  => { // submitted, but not yet accepted
                    Self::deposit_event(RawEvent::ErrorNotApproved(h));
                    return Err("The demander has not approved the work yet!");
                },
                (true, true) => {
                    
                    // Validate that the hash is indeed owned by the buyer
                    match Self::check_ref_owner(o.clone(), h) {
                        true => {
                            // get beneficiary from hash
                            let details =  Self::prefunding_hash_owner(&h).ok_or("Error getting details from hash")?;        
                            let prefunding = <Prefunding<T>>::get(&h);
                            
                            // get prefunding amount for posting to accounts
                            let prefunding = Self::prefunding(&h).ok_or("Error")?;
                            let amount = prefunding.0;
                            let converted_amount = <T::Conversions as Convert<BalanceOf<T>,AccountBalance>>::convert(amount);
                            let increase_amount: AccountBalance = converted_amount.abs();
                            let decrease_amount: AccountBalance = -converted_amount.abs();
                            
                            
                            let current_block = <system::Module<T>>::block_number();

                            let account_1: Account = 120200030000000; // 120200030000000	Debit  decrease Accounts payable
                            let account_2: Account = 110100050000000; // 110100050000000	Credit decrease Totem Runtime Deposit (Escrow)
                            let account_3: Account = 360600020000000; // 360600020000000	Credit decrease Runtime Ledger by Module
                            let account_4: Account = 360600060000000; // 360600060000000	Credit decrease Runtime Ledger Control
                            let account_5: Account = 360600030000000; // 360600030000000	Credit decrease Purchase Ledger by Vendor
                            let account_6: Account = 360600070000000; // 360600070000000	Credit decrease Purchase Ledger Control

                            let account_7: Account = 110100040000000;  // 110100040000000	Debit  increase XTX Balance
                            let account_8: Account = 110100080000000;  // 110100080000000	Credit decrease Accounts receivable (Sales Control Account or Trade Debtor's Account)
                            let account_9: Account = 360600010000000;  // 360600010000000	Credit decrease Sales Ledger by Payer
                            let account_10: Account = 360600050000000; // 360600050000000	Credit decrease Sales Ledger Control

                            // Keys for posting
                            // Buyer
                            let mut forward_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(10);
                            forward_keys.push((o.clone(), account_1, decrease_amount, true, h, current_block));           
                            forward_keys.push((o.clone(), account_2, decrease_amount, false, h, current_block));          
                            forward_keys.push((o.clone(), account_3, decrease_amount, false, h, current_block));          
                            forward_keys.push((o.clone(), account_4, decrease_amount, false, h, current_block));          
                            forward_keys.push((o.clone(), account_5, decrease_amount, false, h, current_block));          
                            forward_keys.push((o.clone(), account_6, decrease_amount, false, h, current_block));          

                            // Seller
                            forward_keys.push((details.2.clone(), account_7, increase_amount, true, h, current_block));   
                            forward_keys.push((details.2.clone(), account_8, decrease_amount, false, h, current_block));  
                            forward_keys.push((details.2.clone(), account_9, decrease_amount, false, h, current_block));  
                            forward_keys.push((details.2.clone(), account_10, decrease_amount, false, h, current_block)); 

                            // Reversal keys in case of errors
                            // Buyer
                            let mut reversal_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(9);
                            reversal_keys.push((o.clone(), account_1, increase_amount, false, h, current_block));
                            reversal_keys.push((o.clone(), account_2, increase_amount, true, h, current_block));
                            reversal_keys.push((o.clone(), account_3, increase_amount, true, h, current_block));
                            reversal_keys.push((o.clone(), account_4, increase_amount, true, h, current_block));
                            reversal_keys.push((o.clone(), account_5, increase_amount, true, h, current_block));
                            reversal_keys.push((o.clone(), account_6, increase_amount, true, h, current_block));

                            // Seller
                            reversal_keys.push((details.2.clone(), account_7, decrease_amount, false, h, current_block));
                            reversal_keys.push((details.2.clone(), account_8, increase_amount, true, h, current_block));
                            reversal_keys.push((details.2.clone(), account_9, increase_amount, true, h, current_block));
                            
                            let track_rev_keys = Vec::<(T::AccountId, Account, AccountBalance, bool, T::Hash, T::BlockNumber)>::with_capacity(9);

                            let _ = Self::handle_multiposting_amounts(o.clone(),forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone())?;
                                   
                        },
                        false => {
                            Self::deposit_event(RawEvent::ErrorNotAllowed(h));
                            return Err("Not the owner");
                        },
                    }
                    
                },
                (false, true) => { // This state is not allowed for this functions
                    Self::deposit_event(RawEvent::ErrorNotAllowed(h));
                    return Err("This function should not be used for this")
                },
                (false, false) => {
                    // Owner has been given permission by beneficiary to release funds
                    Self::deposit_event(RawEvent::ErrorNotAllowed(h));
                    return Err("Funds locked for intended purpose by both parties.")
                    
                },
                _ => {
                    Self::deposit_event(RawEvent::ErrorReleaseState(h));
                    return Err("Error fetching release state");
                },
            }
            
            // Add status processing
            let new_status: Status = 500; // settled(500), can no longer be invoiced, 
            <ReferenceStatus<T>>::insert(&h, new_status);
            
            Ok(())
        }
        
        // Check should be made for available balances, and if the amount submitted is more than the invoice amount. 
        // Settles invoice by updates to various relevant accounts and transfer of funds 
        fn settle_unfunded_invoice() -> Result {
            Ok(())
        }
    }
    
    decl_event!(
        pub enum Event<T>
        where
        AccountId = <T as system::Trait>::AccountId,
        BlockNumber = <T as system::Trait>::BlockNumber,
        Hash = <T as system::Trait>::Hash,
        {
            LegderUpdate(AccountId, Account, AccountBalance),
            PrefundingDeposit(AccountId, AccountBalance, BlockNumber),
            PrefundingCancelled(AccountId, Hash),
            PrefundingLockSet(AccountId, Hash),
            ErrorLockNotAllowed(Hash),
            PrefundingCompleted(AccountId),
            InvoiceIssued(Hash),
            ErrorOverflow(Account),
            ErrorGlobalOverflow(),
            ErrorInsufficientFunds(AccountId),
            ErrorInError(AccountId),
            ErrorNotAllowed(Hash),
            ErrorNotApproved(Hash),
            ErrorDeadlineInPlay(AccountId, Hash),
            ErrorFundsInPlay(AccountId),
            ErrorNotOwner(AccountId, Hash),
            ErrorHashDoesNotExist(Hash),
            ErrorReleaseState(Hash),
            ErrorGettingPrefundData(Hash),
            ErrorTransfer(AccountId, AccountId),
        }
    );