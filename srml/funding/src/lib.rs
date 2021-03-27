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
// This is the Funding / Crowdsale Module for Totem
//********************************************************//

#![cfg_attr(not(feature = "std"), no_std)]

use parity_codec::{Decode, Encode};
// use codec::{ Encode, Decode }; // v2

use srml_support::{
    decl_event, decl_module, decl_storage, dispatch::Result, StorageMap,
    StorageValue,
};
//v1
// use frame_support::{decl_event, decl_error, decl_module, decl_storage, dispatch::DispatchResult, weights::{Weight, DispatchClass}, StorageValue, StorageMap}; // v2

use system::{self, ensure_root, ensure_signed};
//v1
// use frame_system::{self}; //v2

use rstd::prelude::*;
//v1
// use sp_std::prelude::*; //v2

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TXKeysT<Hash> {
    pub tx_uid: Hash,
}

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    // type Bonsai: Storing<Self::Hash>;
}

decl_storage! {
    trait Store for Module<T: Trait> as Funding {
        /// Defines if the transfer mechanism is open yet
        TransferStatus get(transfer_status) config(): bool = false;
        /// The Maximum Quantity of Coins that can be minted
        MaxlIssuance get(max_issuance) config(): u128 = 161_803_398_875u128;
        /// Initially 45% of Supply (Reserved Funds).
        UnIssued get(unissued) config(): u128 = 72_811_529_493u128;
        /// Initially 55% of Supply Reduces as funds distributed.
        Issued get(issued) config(): u128 = 88_991_869_382u128;
        // Controller of funds (Live Accounting Association Account)
        Controller get(controller): T::AccountId;
        // The number of coins distributed. It should equal the sum in AccountIdBalances.
        TotalDistributed get(total_distributed): u128;
        // Place to store investors accountids with balances
        AccountIdBalances get(account_id_balances): map T::AccountId => Option<u128>;
        // List of account Ids who have tokens (updated when  token value is 0)
        HoldersAccountIds get(holders_account_ids): Vec<T::AccountId>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// Super User sets the controller account.
        fn set_controller_account(origin, controller: T::AccountId) -> Result {
            // Only Sudo
            let _who = ensure_root(origin)?;

            // abandon if this is the same controller
            if controller == Self::controller() {
                Self::deposit_event(RawEvent::ErrorSameController());
                return Err("No need to change the same controller");
            } else {
                // remove any existing controller
                <Controller<T>>::take();
                // insert new controller
                <Controller<T>>::put(controller);
            };

            Ok(())
        }
        /// Super User sets the transfers to open or closed.
        fn set_transfer_status(origin) -> Result {
            let _who = ensure_root(origin)?;

            match Self::transfer_status() {
                true => <TransferStatus<T>>::put(false),
                false => {
                    // check to see that everything has been setup before allowing transfers
                    match Self::check_setup() {
                        true => <TransferStatus<T>>::put(true),
                        false => {
                            Self::deposit_event(RawEvent::ErrorControllerNotSet());
                            return Err("Cannot open transfers when controller not set.");
                        },
                    }
                },
            }

            Ok(())
        }
        /// Super User can only mint coins if transfers are disabled
        fn mint_coins(origin, quantity: u128) -> Result {
            let _who = ensure_root(origin)?;

            let mut supply: u128 = Self::max_issuance();
            let mut unissued: u128 = Self::unissued();

            match Self::transfer_status() {
                true => {
                    // cannot mint coins
                    Self::deposit_event(RawEvent::ErrorCannotMintCoins());
                    return Err("Cannot mint whilst transfers open");
                },
                false => {
                    match supply.checked_add(quantity) {
                        Some(s) => supply = s,
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow());
                            return Err("Minting Overflowed!");
                        },
                    }
                    match unissued.checked_add(quantity) {
                        Some(u) => unissued = u,
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow());
                            return Err("Minting Overflowed!");
                        },
                    }
                },
            }

            // Update unissued account with new balance
            <UnIssued<T>>::take();
            <UnIssued<T>>::put(unissued);
            // Update Max Supply
            <MaxlIssuance<T>>::take();
            <MaxlIssuance<T>>::put(supply);

            Ok(())
        }
        /// Super User can move from unissued to issued coins if transfers are disabled
        fn rebalance_issued_coins(origin, amount: u128) -> Result {
            let _who = ensure_root(origin)?;
            let mut unissued = Self::unissued();
            let mut issued = Self::issued();

            // check that the amount is not greater than the available funds
            if amount > unissued {
                // This is not allowed
                Self::deposit_event(RawEvent::ErrorInsufficientFunds());
                return Err("Insufficient funds to rebalance.");
            } else if amount <= unissued {
                match unissued.checked_sub(amount) {
                    Some(n) => unissued = n,
                    None => {
                        // This error should never happen.
                        Self::deposit_event(RawEvent::ErrorOverflow());
                        return Err("Overflow error");
                    },
                };
                match issued.checked_add(amount) {
                    Some(n) => issued = n,
                    None => {
                        // This error should never happen.
                        Self::deposit_event(RawEvent::ErrorOverflow());
                        return Err("Overflow error");
                    },
                };
            };
            <UnIssued<T>>::take();
            <UnIssued<T>>::put(unissued);
            <Issued<T>>::take();
            <Issued<T>>::put(issued);
            Ok(())
        }
        /// Only the controller can do the initial distribution
        fn distribute(origin, to: T::AccountId, amount: u128) -> Result {
            let who = ensure_signed(origin)?;
            // ensure that this is the controller account
            if who == Self::controller() {
                // This is the controller and funds can be distributed.
                ();
            } else {
                Self::deposit_event(RawEvent::ErrorNotController());
                return Err("You are not the controller");
            }
            // Ensure that the amount to send is less the available funds.
            let mut issued: u128 = Self::issued();
            let total_distributed: u128;
            let mut new_balance: u128 = 0u128;

            if amount > issued {
                // This is not allowed
                Self::deposit_event(RawEvent::ErrorInsufficientFunds());
                return Err("Insufficient funds to rebalance.");
            } else if amount <= issued {
                ();
            };
            match issued.checked_sub(amount) {
                Some(i) => issued = i,
                None => {
                    Self::deposit_event(RawEvent::ErrorOverflow());
                    return Err("Minting Overflowed!");
                },
            }
            match Self::account_id_balances(&to) {
                Some(b) => {
                    match b.checked_add(amount) {
                        Some(n) => {
                            new_balance = n;
                            <AccountIdBalances<T>>::take(&to);
                        },
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow());
                            return Err("Minting Overflowed!");
                        },
                    }
                },
                None => (),
            }
            match Self::total_distributed().checked_add(amount) {
                Some(n) => total_distributed = n,
                None => {
                    Self::deposit_event(RawEvent::ErrorOverflow());
                    return Err("Minting Overflowed!");
                },
            }
            <Issued<T>>::take();
            <Issued<T>>::put(issued);
            <AccountIdBalances<T>>::insert(&to, new_balance);
            <TotalDistributed<T>>::take();
            <TotalDistributed<T>>::put(total_distributed);
            <HoldersAccountIds<T>>::mutate(|holders_account_ids| holders_account_ids.push(to));
            Ok(())
        }
        /// This function transfers funds between accounts (only when opened)
        fn transfer(origin, to: T::AccountId, amount: u128) -> Result {
            let from = ensure_signed(origin)?;

            // are transfers open?
            if !Self::transfer_status() {
                Self::deposit_event(RawEvent::ErrorTransfersNotOpen());
                return Err("Transfers not open.");
            } else {
                let mut new_sender_balance: u128;
                let mut new_receiver_balance: u128 = 0u128;
                // Get the balance of sender
                match Self::account_id_balances(&from) {
                    Some(b) => new_sender_balance = b,
                    None => {
                        Self::deposit_event(RawEvent::ErrorInsufficientFunds());
                        return Err("Insufficient funds to transfer.");
                    },
                }
                match Self::account_id_balances(&to) {
                    Some(b) => new_receiver_balance = b,
                    None => (),
                }
                if new_sender_balance < amount {
                    Self::deposit_event(RawEvent::ErrorInsufficientFunds());
                    return Err("Insufficient funds to transfer.");
                } else if new_sender_balance > amount{
                    // reduce balance on sender
                    match new_sender_balance.checked_sub(amount) {
                        Some(n) => {
                            new_sender_balance = n;
                        },
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow());
                            return Err("Reduction Overflowed!");
                        },
                    }
                    // increase balance on receiver
                    match new_receiver_balance.checked_add(amount) {
                        Some(n) => {
                            new_receiver_balance = n;
                        },
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow());
                            return Err("Adding Overflowed!");
                        },
                    }
                    <AccountIdBalances<T>>::take(&from);
                    <AccountIdBalances<T>>::insert(&from, new_sender_balance);
                    <AccountIdBalances<T>>::take(&to);
                    <AccountIdBalances<T>>::insert(&to, new_receiver_balance);
                    // Following ensures that only one entry exists in the list of addresses with funds.
                    <HoldersAccountIds<T>>::mutate(|holders_account_ids| holders_account_ids.retain(|t| {t != &to}));
                    <HoldersAccountIds<T>>::mutate(|holders_account_ids| holders_account_ids.push(to));
                } else {
                    let mut new_receiver_balance: u128 = 0u128;
                    match Self::account_id_balances(&to) {
                        Some(b) => new_receiver_balance = b,
                        None => (),
                    }
                    
                    match new_receiver_balance.checked_add(amount) {
                        Some(n) => {
                            new_receiver_balance = n;
                        },
                        None => {
                            Self::deposit_event(RawEvent::ErrorOverflow());
                            return Err("Adding Overflowed!");
                        },
                    }
                    // balance of sender will be 0 remove from table
                    <AccountIdBalances<T>>::remove(&from);
                    <HoldersAccountIds<T>>::mutate(|holders_account_ids| holders_account_ids.retain(|f| {f != &from}));
                    // increase balance on receiver
                    <AccountIdBalances<T>>::take(&to);
                    <AccountIdBalances<T>>::insert(&to, new_receiver_balance);
                    // Following ensures that only one entry exists in the list of addresses with funds.
                    <HoldersAccountIds<T>>::mutate(|holders_account_ids| holders_account_ids.retain(|t| {t != &to}));
                    <HoldersAccountIds<T>>::mutate(|holders_account_ids| holders_account_ids.push(to));
                    
                };
            };
            Ok(())
        }
        
    }
}

impl<T: Trait> Module<T> {
    #[allow(dead_code)]
    // check if all the setup actions have been done
    fn check_setup() -> bool {
        let mut answer: bool = false;

        // has controller been set
        if <Controller<T>>::exists() {
            answer = true;
        } else {
            ();
        };
        return answer;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        SuccessMessage(AccountId),
        /// You cannot change a controller to the same controller
        ErrorSameController(),
        /// You are not the controller
        ErrorNotController(),
        /// Cannot open transfers when controller not set
        ErrorControllerNotSet(),
        /// Cannot mint whilst transfers open
        ErrorCannotMintCoins(),
        /// Minting Overflowed
        ErrorOverflow(),
        /// Insufficient funds to rebalance.
        ErrorInsufficientFunds(),
        /// Transfers not open.
        ErrorTransfersNotOpen(),
    }
);