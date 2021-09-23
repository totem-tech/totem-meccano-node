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
// This is the Transfer Module for Totem. It essentially replaces the existing
// transfer() function in the balances module by adding an additional tracking 
// mechanism for when the user is offline. It also allows us to manage distribution of funds
// from the faucet so that funds are not resent to users when there is a network failure.
//********************************************************//

use support::{
    decl_event, 
    decl_module, 
    dispatch::Result
};
//v1
// use frame_support::{decl_event, decl_error, decl_module, decl_storage, dispatch::DispatchResult, weights::{Weight, DispatchClass}, StorageValue, StorageMap}; // v2

use system::{self, ensure_signed};
//v1
// use frame_system::{self}; //v2

use rstd::prelude::*;
//v1
// use sp_std::prelude::*; //v2
use runtime_primitives::traits::{Convert};
use support::traits::{Currency};
//v1 
// use frame_support::Traits{Currency}; // v2
// Totem Pallets
use accounting::{ Posting };

// Totem Trait Types
type AccountOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber,<T as accounting::Trait>::CoinAmount>>::Account;
type AccountBalanceOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber,<T as accounting::Trait>::CoinAmount>>::LedgerBalance;

// Other trait types
type CurrencyBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

use crate::bonsai_traits::{ Storing };

pub trait Trait: system::Trait + balances::Trait + accounting::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Currency: Currency<Self::AccountId>;
    type TransferConversions: Convert<Self::Balance, CurrencyBalanceOf<Self>>
    + Convert<Self::Balance, AccountBalanceOf<Self>>
    + Convert<Self::Balance, i128>
    + Convert<u64, AccountOf<Self>>
    + Convert<CurrencyBalanceOf<Self>, i128>
    + Convert<i128, AccountBalanceOf<Self>>;
    type Bonsai: Storing<Self::Hash>;
    type Accounting: Posting<Self::AccountId,Self::Hash,Self::BlockNumber,Self::CoinAmount>;
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// Transfers funds!
        /// This is a direct transfer, with no specific invoice attached to it.
        fn network_currency(
            origin, 
            to: T::AccountId, 
            #[compact] payment_amount: T::Balance,
            tx_uid: T::Hash 
        ) -> Result {
            let from = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::start_tx(tx_uid.clone())?;
            
            // Convert incoming amount to currency for transfer
            let amount: CurrencyBalanceOf<T> = <T::TransferConversions as Convert<T::Balance, CurrencyBalanceOf<T>>>::convert(payment_amount.clone());
            let posting_amount: i128 = <T::TransferConversions as Convert<T::Balance, i128>>::convert(payment_amount);
            let account_1: AccountOf<T> = <T::TransferConversions as Convert<u64, AccountOf<T>>>::convert(110100040000000u64); // debit increase - credit decrease 110100040000000 XTX Balance
            
            // Convert this for the inversion
            let to_invert: i128 = 0i128 - posting_amount.clone();

            let increase_amount: AccountBalanceOf<T> = <T::TransferConversions as Convert<i128, AccountBalanceOf<T>>>::convert(posting_amount);
            let decrease_amount: AccountBalanceOf<T> = <T::TransferConversions as Convert<i128, AccountBalanceOf<T>>>::convert(to_invert);
            
            // This sets the change block and the applicable posting period. For this context they will always be
            // the same.
            let current_block = <system::Module<T>>::block_number(); // For audit on change
            let current_block_dupe = current_block.clone(); // Applicable period for accounting
    
            // Generate dummy Hash reference (it has no real bearing but allows posting to happen)
            let tx_ref_hash: T::Hash = tx_uid.clone();
                
            // Keys for posting by payer
            let mut forward_keys = Vec::<(
                T::AccountId,T::AccountId,AccountOf<T>,AccountBalanceOf<T>,bool,T::Hash,T::BlockNumber,T::BlockNumber,
            )>::with_capacity(2);
            
            // Sender
            forward_keys.push((from.clone(),to.clone(),account_1,decrease_amount,true,tx_ref_hash,current_block,current_block_dupe,));
            // Receiver
            forward_keys.push((to.clone(),from.clone(),account_1,increase_amount,false,tx_ref_hash,current_block,current_block_dupe,));
            
            // Reversal keys in case of errors
            let mut reversal_keys = Vec::<(
                T::AccountId,T::AccountId,AccountOf<T>,AccountBalanceOf<T>,bool,T::Hash,T::BlockNumber,T::BlockNumber,
            )>::with_capacity(1);
            reversal_keys.push((from.clone(),to.clone(),account_1,increase_amount,false,tx_ref_hash,current_block,current_block_dupe,));
    
            let track_rev_keys = Vec::<(
                T::AccountId,T::AccountId,AccountOf<T>,AccountBalanceOf<T>,bool,T::Hash,T::BlockNumber,T::BlockNumber,
            )>::with_capacity(2);
    
            match <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber,T::CoinAmount>>::handle_multiposting_amounts(forward_keys.clone(),reversal_keys.clone(),track_rev_keys.clone()) {
                Ok(_) => (),
                Err(_e) => {
                    Self::deposit_event(RawEvent::ErrorPostingAccounts(tx_uid));
                    return Err("An error occured posting to accounts");
                },
            }

            match T::Currency::transfer(&from, &to, amount) {
                Ok(_) => (),
                Err(_) => {
                    Self::deposit_event(RawEvent::ErrorDuringTransfer(tx_uid));
                    return Err("Error during transfer");
                },
            }
            <<T as Trait>::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;
            Ok(())
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
    Hash = <T as system::Trait>::Hash,
    {
        /// There was an error calling the transfer function in balances
        ErrorDuringTransfer(Hash),
        ErrorPostingAccounts(Hash),
    }
);