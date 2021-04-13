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

// Other trait types
type CurrencyBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

use crate::bonsai_traits::{ Storing };

pub trait Trait: system::Trait + balances::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Currency: Currency<Self::AccountId>;
    type TransferConversions: Convert<Self::Balance, CurrencyBalanceOf<Self>>;
    type Bonsai: Storing<Self::Hash>;
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// Transfers funds!
        fn transfer(
            origin, 
            to: T::AccountId, 
            #[compact] payment_amount: T::Balance,
            tx_uid: T::Hash 
        ) -> Result {
            let from = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::start_tx(tx_uid.clone())?;
            
            // Convert incoming amount to currency for transfer
            let amount: CurrencyBalanceOf<T> = <T::TransferConversions as Convert<T::Balance, CurrencyBalanceOf<T>>>::convert(payment_amount);

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
    }
);