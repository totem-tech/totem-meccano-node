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

use parity_codec::Encode;
use rstd::prelude::*;
use runtime_primitives::traits::{Convert, Hash};
use substrate_primitives::H256;
use support::traits::{Currency, LockIdentifier, LockableCurrency, WithdrawReason};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageValue};
use system::{self, ensure_root, ensure_signed};

// type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + balances::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Currency: Currency<Self::AccountId>
        + LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
    type CrowdsaleConversions: Convert<Self::Balance, u128>
        + Convert<u64, Self::BlockNumber>
        + Convert<Self::BlockNumber, u64>
        + Convert<u128, Self::Balance>;
}

decl_storage! {
    trait Store for Module<T: Trait> as CrowdSaleModule {
        // Parameters for crowdsale (should only be set by system root)
        // Start and End Blocks
        CrowdsaleDuration get(crowdsale_duration): Option<(T::BlockNumber,T::BlockNumber)>;
        // Gap Between lock releases, and last locked block number
        LockGap get(lock_gap): Option<(T::BlockNumber, T::BlockNumber)>;
        // Sets Faucet Address (to check who is requesting)
        Faucet get(faucet): T::AccountId;
        // Maps levels  0,1,2,3,4,5,6,7,8 (8 = overflow level) to max amount of contribution
        Levels get(levels): map u16 => Option<u128>;
        // Maps levels to multipliers
        Multipliers get(multipliers): map u16 => Option<T::Balance>;

        // Main storage
        // Maps contributor to their multiplier level
        Contributor get(contributor): map T::AccountId => Option<(u16, T::Balance)>;
        // Release buckets for managing release schedule.
        // Total, release 0,1,2,3,4, overflow (all summed should equal the total)
        //
        ReleaseBuckets get(release_buckets): map T::AccountId => Option<(T::Balance,T::Balance,T::Balance,T::Balance,T::Balance,T::Balance,T::Balance)>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}

impl<T: Trait> Module<T> {
    fn start_parameters_set() -> bool {
        <CrowdsaleDuration<T>>::exists()
    }

    fn release_blocks_set() -> bool {
        <LockGap<T>>::exists()
    }

    fn process_shout_out() -> Result {
        Ok(())
    }

    fn process_referral() -> Result {
        Ok(())
    }

    fn process_crowdsale() -> Result {
        Ok(())
    }

    fn set_start_and_end_blocks(
        start: T::BlockNumber,
        end: T::BlockNumber,
        release: T::BlockNumber,
    ) -> Result {
        if Self::start_parameters_set() {
            <CrowdsaleDuration<T>>::take();
            if Self::start_parameters_set() {
                <LockGap<T>>::take();
            } else {
                (); // Skip
            }
        } else {
            (); // Do nothing
        };
        // Calculate the last block number of the locks
        let current_block: u64 = <T::CrowdsaleConversions as Convert<T::BlockNumber, u64>>::convert(
            <system::Module<T>>::block_number(),
        );
        let end_block_clone = end.clone();
        let mut end_block_converted: u64 =
            <T::CrowdsaleConversions as Convert<T::BlockNumber, u64>>::convert(end_block_clone);
        let release_distance: u64 =
            <T::CrowdsaleConversions as Convert<T::BlockNumber, u64>>::convert(release);
        if release_distance > 0u64 {
            let last_lock: u64 = release_distance * 4;
            end_block_converted = last_lock + end_block_converted;
            let last_lock_block: T::BlockNumber = <T::CrowdsaleConversions as Convert<
                u64,
                T::BlockNumber,
            >>::convert(end_block_converted);
            // set the storage values
            <LockGap<T>>::put((release, last_lock_block));
            <CrowdsaleDuration<T>>::put((start, end));
        } else {
            // return because this is an error
            Self::deposit_event(RawEvent::ErrorEndLockZero());
            return Err("End lock value cannot be zero");
        };
        Ok(())
    }

    fn test_max_for_level(n: &mut u128, l: &mut u16, max: u128) -> Result {
        if *n > max {
            // substract from new balance and increment level
            *n -= max;
            *l += 1u16;
        } else if *n < max {
            // nothing changes
            ();
        } else if *n == max {
            // just increment the level
            *l += 1u16;
        }
        Ok(())
    }

    fn set_crowdsale_lock(c: T::AccountId, a: T::Balance) -> Result {
        // Faucet sends transaction of contribution amount in XTX
        // This function adds that amount to the total contributed and recalculates the multiplier level that has been achieved
        // Then recalculates the release schedule

        // Copy contribution amount
        let mut new_contribution_total: T::Balance = a.clone();

        let mut new_contribution_total_for_storage: T::Balance = a.into(); // Initialised with dummy value
        let mut level: u16 = 0u16; //Initialised with starting value
        let mut existing_balance: T::Balance;

        match Self::contributor(c) {
            Some(l) => {
                // This contributor has received funds already.

                // set the existing balance
                existing_balance = l.1.clone();

                // sum the incoming amount to the existing balance.
                new_contribution_total += l.1;

                // DO NOT FORGET TO UPDATE STORAGE
                new_contribution_total_for_storage = new_contribution_total.clone();

                // convert new balance to number
                let mut new_contribution_total_converted: u128 =
                    <T::CrowdsaleConversions as Convert<T::Balance, u128>>::convert(
                        new_contribution_total,
                    );
                // Update multiplier
                level = l.0;

                while level < 9u16 {
                    match level {
                        0u16 | 1u16 | 2u16 | 3u16 | 4u16 | 5u16 | 6u16 | 7u16 => {
                            // get maximum amount for this level
                            match Self::levels(level) {
                                Some(m) => {
                                    match Self::test_max_for_level(
                                        &mut new_contribution_total_converted,
                                        &mut level,
                                        m,
                                    ) {
                                        Ok(_) => {
                                            ();
                                        }
                                        Err(_e) => {
                                            // should never happen
                                            ();
                                        }
                                    }
                                }
                                None => {
                                    // Should not happen as parameters must be set
                                    // return with error
                                    ();
                                }
                            }
                        }
                        _ => {
                            // if the level is greater than 7, do not increment further, simply add the new balance
                            ();
                        }
                    }
                }
            }
            None => {
                // This is the first time this contributor has received funds
                ();
            }
        }

        // select multiplier based on latest known level
        let mut multiplier: u128 = 0u128; // dummy initial value 
         match Self::multipliers(level) {
            Some(m) => {
                // convert maximum amount to u128 for comparison
                multiplier = <T::CrowdsaleConversions as Convert<T::Balance, u128>>::convert(m);
            }
            None => {
                // This should never happen as parameters must be set
                // return with error
                return Err("Should not happen");
            }
        };
        
        // calculate total allocation of multiplier
        let mut total_converted: u128 = <T::CrowdsaleConversions as Convert<T::Balance, u128>>::convert(new_contribution_total);
        let total = match total_converted.checked_mul(multiplier) {
            Some(t) => t,
            None => {
                return Err("Overflow occured");
            },
        };
        let total_allocation: T::Balance = <T::CrowdsaleConversions as Convert<u128,T::Balance>>::convert(total);
        // Total, release 0,1,2,3,4, overflow (all summed should equal the total)
        // (T::Balance,T::Balance,T::Balance,T::Balance,T::Balance,T::Balance,T::Balance)

        // Re-calculate the release schedule for this identity

        // at this point faucet has not transferred funds
        // This function handles the notation of the funds to be locked and then takes the funds from the faucet

        Ok(())
    }

    fn check_can_withdraw() -> Result {
        Ok(())
    }

    fn withdraw() -> Result {
        Ok(())
    }
}

// impl<T: Trait> Storing<T::Hash> for Module<T> {

// }

decl_event!(
    pub enum Event<T>
    where
        Hash = <T as system::Trait>::Hash,
    {
        /// The submitted end lock value cannot be zero.
        ErrorEndLockZero(),
        /// Unused error
        ErrorUnknownType(Hash),
    }
);
