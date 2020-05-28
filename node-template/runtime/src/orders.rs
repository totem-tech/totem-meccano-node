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
// This is the Totem Orders Module 
//********************************************************//

// The orders module supports creation of purchase orders and tasks and other types of market order.
// A basic workflow is as follows:
// * In general orders are assigned to a partner that the ordering identity already knows and is required to be accepted.
// * Orders can be made without already knowing the seller - these are called market orders
// * The order can be prefunded by calling into the prefunding module, which updates the accounting ledgers.
// * Once the order is accepted, the work must begin, and once completed, the vendor sets the state to completed.
// * The completion state also generates the invoice, and relevant accounting postings for both the buyer and the seller.
// * The completed work is then approved by the buyer (or disputed or rejected). An approval triggers the release of prefunds and 
// the invoice is marked as settled in the accounts for both parties

In the first 

use support::{
    decl_event, 
    decl_module, 
    decl_storage, 
    dispatch::Result, 
    ensure, 
    StorageMap
};
use system::ensure_signed;
use parity_codec::{Decode, Encode};
use runtime_primitives::traits::Hash;
use rstd::prelude::*;

pub trait Trait: projects::Trait + system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}


decl_storage! {
    trait Store for Module<T: Trait> as OrdersModule {
        Orders get(orders): map T::AccountId => Vec<T::Hash>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}

impl<T: Trait> Module<T> {
    fn create_simple_order() -> Result {
        Ok(())
    }
}

decl_event!(
    pub enum Event<T>
    where
    // AccountId = <T as system::Trait>::AccountId,
    Hash = <T as system::Trait>::Hash
    {
        Dummy(Hash),
    }
);
