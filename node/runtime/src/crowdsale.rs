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
use runtime_primitives::traits::{Convert, Hash, Zero};
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
        Multipliers get(multipliers): map u16 => Option<u128>;

        // Main storage
        // Maps contributor to their multiplier level
        Contributor get(contributor): map T::AccountId => Option<(u16, u128)>;
        // Release buckets for managing release schedule.
        // Total, release 0,1,2,3,4, overflow (all summed should equal the total)
        //
        ReleaseBuckets get(release_buckets): map T::AccountId => Option<(u128,u128,u128,u128,u128,u128,u128)>;
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

    fn process_level_5_up(
        a: u128,
        s: u128,
        t: u128,
        nrs: &mut (u128, u128, u128, u128, u128, u128, u128),
        z: u128,
    ) -> Result {
        if t > a {
            return Err("Mismatch between level and allocation amount");
        } else if t < a {
            let remainder = match t.checked_sub(s) {
                Some(o) => o,
                None => return Err("Mismatch between remainder and split amount"),
            };
            if remainder > s {
                // This needs to be divided further - at least once
                let over = match remainder.checked_sub(s) {
                    Some(r) => {
                        if r > s {
                            // still too big, split again
                            let pre_over = match remainder.checked_sub(s) {
                                Some(p) => {
                                    let final_over = match remainder.checked_sub(s) {
                                        Some(f) => <T::CrowdsaleConversions as Convert<
                                            u128,
                                            T::Balance,
                                        >>::convert(
                                            f
                                        ),
                                        None => {
                                            return Err(
                                                "Mismatch between remainder and split amount",
                                            )
                                        }
                                    };
                                    nrs = (ta, s, s, s, s, final_over, z);
                                }
                                None => return Err("Mismatch between remainder and split amount"),
                            };
                        } else if r <= s {
                            // This should not happen because the over amount must always be greater than split
                            return Err("Mismatch between remainder and split amount");
                        };
                    }
                    None => return Err("Mismatch between remainder and split amount"),
                };
            } else if remainder < s {
                // no need to split further
                let over: T::Balance =
                    <T::CrowdsaleConversions as Convert<u128, T::Balance>>::convert(remainder);
                nrs = (ta, s, s, s, s, over, z);
            } else if remainder == s {
                return Err(
                    "This should not happen here! It should happen in the outer if statement",
                );
            };
        } else if t == a {
            nrs = (ta, s, s, s, s, s, z);
        };

        Ok(())
    }

    fn set_crowdsale_lock(c: T::AccountId, a: u128) -> Result {
        // Faucet sends transaction of contribution amount in XTX
        // This function adds that amount to the total contributed and recalculates the multiplier level that has been achieved
        // Then recalculates the release schedule
        const BALANCE_ZERO: u128 = 0u128;
        const L1: u16 = 0u16;
        const L2: u16 = 1u16;
        const L3: u16 = 2u16;
        const L4: u16 = 3u16;
        const L5: u16 = 4u16;
        const L6: u16 = 5u16;
        const L7: u16 = 6u16;
        const L8: u16 = 7u16;
        const L10: u16 = 9u16;
        // These constants are hard coded for the moment. They should be made into parameters
        const L1ALLOC: u128 = 6449400u128; // XTX
        const L2ALLOC: u128 = 128988000u128; // XTX
        const L3ALLOC: u128 = 322470000u128; // XTX
        const L4ALLOC: u128 = 644940000u128; // XTX
        const L5ALLOC: u128 = 1612350000u128; // XTX
        const L6ALLOC: u128 = 3224700000u128; // XTX
        const L7ALLOC: u128 = 4837050000u128; // XTX
        const L8ALLOC: u128 = 6449400000u128; // XTX
        const L1SPLIT: u128 = 6449400u128; // XTX
        const L2SPLIT: u128 = 64494000u128; // XTX
        const L3SPLIT: u128 = 107490000u128; // XTX
        const L4SPLIT: u128 = 161235000u128; // XTX
        const L5SPLIT: u128 = 322470000u128; // XTX
        const L6SPLIT: u128 = 644940000u128; // XTX
        const L7SPLIT: u128 = 967410000u128; // XTX
        const L8SPLIT: u128 = 1289880000u128; // XTX

        // Copy contribution amount
        let mut new_contribution_total: u128 = a.clone();

        let mut new_contribution_total_for_storage: u128 = a.into(); // Initialised with dummy value
        let mut level: u16 = L1; //Initialised with starting value
        let mut original_contribution_balance: u128;

        match Self::contributor(c) {
            Some(l) => {
                // This contributor has received funds already.

                // set the existing balance
                original_contribution_balance = l.1.clone();

                // sum the incoming amount to the existing balance to get the new total contribution.
                new_contribution_total += l.1;

                // DO NOT FORGET TO UPDATE STORAGE
                new_contribution_total_for_storage = new_contribution_total.clone();

                // Update multiplier level
                level = l.0;

                while level < L10 {
                    match level {
                        L1 | L2 | L3 | L4 | L5 | L6 | L7 | L8 => {
                            // get maximum amount for this level
                            match Self::levels(level) {
                                Some(m) => {
                                    match Self::test_max_for_level(
                                        &mut new_contribution_total,
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
        let mut multiplier: u128 = BALANCE_ZERO; // dummy initial value
        let multiplier = match Self::multipliers(level) {
            Some(m) => m,
            None => {
                // This should never happen as parameters must be set
                // return with error
                return Err("Should not happen");
            }
        };

        // calculate total allocation of multiplier
        let total_allocation = match new_contribution_total.checked_mul(multiplier) {
            Some(t) => t,
            None => {
                return Err("Overflow occured");
            }
        };

        // Re-calculate the release schedule for this identity
        // TODO Fill release bucket allocations depending on level.
        // Total, release 0,1,2,3,4, overflow (all summed should equal the total)
        // (T::Balance,T::Balance,T::Balance,T::Balance,T::Balance,T::Balance,T::Balance)
        // i.e. divide the total allocation by x according to level.
        let mut new_release_schedule = (
            total_allocation,
            BALANCE_ZERO,
            BALANCE_ZERO,
            BALANCE_ZERO,
            BALANCE_ZERO,
            BALANCE_ZERO,
            BALANCE_ZERO,
        );
        match level {
            L1 => {
                // If the level is 1 then the total allocation amount should not be greater than 6449400 XTX
                if total_allocation > L1ALLOC {
                    return Err("Mismatch between level and allocation amount");
                } else if total_allocation <= L1ALLOC {
                    new_release_schedule.1 = L1SPLIT;
                };
            }
            L2 => {
                if total_allocation > L2ALLOC {
                    return Err("Mismatch between level and allocation amount");
                } else if total_allocation < L2ALLOC {
                    match total_allocation.checked_sub(L2SPLIT) {
                        Some(o) => {
                            new_release_schedule.1 = L2SPLIT;
                            new_release_schedule.2 = o;
                        }
                        None => return Err("Mismatch between level and allocation amount"),
                    };
                } else if total_allocation == L2ALLOC {
                    new_release_schedule.1 = L2SPLIT;
                    new_release_schedule.2 = L2SPLIT;
                };
            }
            L3 => {
                if total_allocation > L3ALLOC {
                    return Err("Mismatch between level and allocation amount");
                } else if total_allocation < L3ALLOC {
                    match total_allocation.checked_sub(L3SPLIT) {
                        Some(o) => {
                            if o > L3SPLIT {
                                // This needs to be divided further - at least once
                                match o.checked_sub(L3SPLIT) {
                                    Some(r) => {
                                        new_release_schedule.1 = L3SPLIT;
                                        new_release_schedule.2 = L3SPLIT;
                                        new_release_schedule.3 = r;
                                    }
                                    None => {
                                        return Err("Mismatch between level and allocation amount")
                                    }
                                };
                            } else if o < L3SPLIT {
                                return Err("Mismatch between level and allocation amount");
                            } else if o == L3SPLIT {
                                return Err("This should not happen here! It should happen in the outer if statement");
                            };
                        }
                        None => return Err("Mismatch between level and allocation amount"),
                    };
                } else if total_allocation == L3ALLOC {
                    new_release_schedule.1 = L3SPLIT;
                    new_release_schedule.2 = L3SPLIT;
                    new_release_schedule.3 = L3SPLIT;
                };
            }
            L4 => {
                if total_allocation > L4ALLOC {
                    return Err("Mismatch between level and allocation amount");
                } else if total_allocation < L4ALLOC {
                    match total_allocation.checked_sub(L4SPLIT) {
                        Some(o) => {
                            if o > L4SPLIT {
                                // This needs to be divided further - at least once
                                let over = match o.checked_sub(L4SPLIT) {
                                    Some(r) => {
                                        if r > L4SPLIT {
                                            // still too big, split again
                                            match o.checked_sub(L4SPLIT) {
                                                Some(f) => {
                                                    new_release_schedule.1 = L4SPLIT;
                                                    new_release_schedule.2 = L4SPLIT;
                                                    new_release_schedule.3 = L4SPLIT;
                                                    new_release_schedule.4 = f;
                                                }
                                                None => return Err("Mismatch between remainder and split amount"),
                                            };
                                        } else if r <= L4SPLIT {
                                            // This should not happen because the over amount must always be
                                            // greater than split
                                            return Err(
                                                "Mismatch between remainder and split amount",
                                            );
                                        };
                                    }
                                    None => return Err("Mismatch between remainder and split amount"),
                                    
                                };
                            } else if o < L4SPLIT {
                                return Err("Mismatch between remainder and split amount");
                            } else if o == L4SPLIT {
                                return Err("This should not happen here! It should happen in the outer if statement");
                            };
                        }
                        None => return Err("Mismatch between remainder and split amount"),
                    };
                } else if total_allocation == L4ALLOC {
                    new_release_schedule.1 = L4SPLIT;
                    new_release_schedule.2 = L4SPLIT;
                    new_release_schedule.3 = L4SPLIT;
                    new_release_schedule.4 = L4SPLIT;
                };
            }
            L5 => {
                match Self::process_level_5_up(
                    L5ALLOC,
                    L5SPLIT,
                    total,
                    total_allocation,
                    &mut new_release_schedule,
                    BALANCE_ZERO,
                ) {
                    Ok(_) => (),
                    Err(_e) => {
                        return Err("Something went wrong");
                    }
                };
            }
            L6 => {
                match Self::process_level_5_up(
                    L6ALLOC,
                    L6SPLIT,
                    total,
                    total_allocation,
                    &mut new_release_schedule,
                    BALANCE_ZERO,
                ) {
                    Ok(_) => (),
                    Err(_e) => {
                        return Err("Something went wrong");
                    }
                };
            }
            L7 => {
                match Self::process_level_5_up(
                    L7ALLOC,
                    L7SPLIT,
                    total,
                    total_allocation,
                    &mut new_release_schedule,
                    BALANCE_ZERO,
                ) {
                    Ok(_) => (),
                    Err(_e) => {
                        return Err("Something went wrong");
                    }
                };
            }
            L8 => {
                match Self::process_level_5_up(
                    L8ALLOC,
                    L8SPLIT,
                    total,
                    total_allocation,
                    &mut new_release_schedule,
                    BALANCE_ZERO,
                ) {
                    Ok(_) => (),
                    Err(_e) => {
                        return Err("Something went wrong");
                    }
                };
            }
            _ => {
                // Todo - deal with the overflow. More money has been allocated
                ();
            }
        };

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
