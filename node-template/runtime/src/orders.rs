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

// Totem Traits
use crate::totem_traits::{ Posting, Encumbrance };

// Totem Trait Types
type AccountOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::Account;
type AccountBalanceOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::AccountBalance;

// Other trait types
// type CurrencyBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

// Module Types
// pub type UnLocked = bool; // 0=Unlocked(false) 1=Locked(true)
pub type OrderStatus = u16; // Generic Status for whatever the HashReference refers to

type Product = Hash;
type UnitPrice = AccountBalanceOf<T>;
type Quantity = AccountBalanceOf<T>;
type OrderItem = Vec<(T::AccountId, Product, UnitPrice, Quantity, T::BlockNumber, T::BlockNumber)>;

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Accounting: Posting<Self::AccountId,Self::Hash,Self::BlockNumber> + Encumbrance<Self::AccountId,Self::Hash,Self::BlockNumber>;
}

decl_storage! {
    trait Store for Module<T: Trait> as OrdersModule {
        OrderOwner get(order_owner): map T::AccountId => Vec<T::Hash>;
        OrderBeneficiary get(order_beneficiary): map T::AccountId => Vec<T::Hash>;
        OrderDetails get(order_owner): map T::Hash  => Vec<(Product, UnitPrice, Quantity)>;
        OrderStatus get(order_status): map T::Hash => Option<OrderStatus>;
        OrderApproved get(order_approved): map T::Hash => Option<bool>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}

impl<T: Trait> Module<T> {
    /// Open an order for a specific AccountId and prefund it. This is equivalent to an encumbrance. 
    /// The amount is the functional currency and conversions are not necessary at this stage of accounting. 
    /// The UI therefore handles presentation or reporting currency translations at spot rate 
    /// This is not for goods.
    /// If the order is open, the the fulfiller is ignored. 
    /// Order type is generally goods (0) or services (1) but is left open for future-proofing 
     fn set_simple_prefunded_service_order(
        commander: T::AccountId, 
        approver: T::AccountId, 
        fulfiller: T::AccountId, 
        buy_or_sell: bool, 
        deadline: Blocknumber, 
        amount: AccountBalanceOf<T>,
        open_closed: bool,
        order_type: u16,
        due_date: T::BlockNumber,
        // order_items: Vec<(Product, UnitPrice, Quantity)>
        order_items: Vec<(Product, UnitPrice, Quantity)>
    ) -> Result {
        
        // Generate Hash for order
        let order_hash = <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber>>::get_pseudo_random_hash(commander.clone(),approver.clone())?;
        // TODO check that it does not already exit
        
        let status: OrderStatus = 0;
        
        // check order auto approved, but if not for the moment return an error
        if commander != approver {
            // The identity ordering this is also the approver, therefore it can be considered approved by default
            <OrderApproved<T>>::insert(approver.clone(), true);    
        } else {
            <OrderApproved<T>>::insert(approver.clone(), false);
            return Err("Cannot submit an order that cannot be approved");
        };
        
        
        
        // Set hash for commander
        <OrderOwner<T>>::insert(commander.clone(), &task_hash);

        // submitted(0), accepted(1), rejected(2), disputed(3), blocked(4), invoiced(5), reason_code(0), reason text.
        // Set Acceptance Status
        <OrderStatus<T>>::insert(&task_hash, status);
        
        // Set Prefunding - do this now, it does not matter if there are errors after this point.
        ensure!(commander != beneficiary, "Beneficiary must be another account");
        <<T as Trait>::Accounting as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::prefunding_for(who, beneficiary, amount.into(), deadline)?;
        
        // Set hash for fulfiller
        <OrderBeneficiary<T>>::insert(fulfiller.clone(), &task_hash);

        // Set details of Order
        <OrderDetails<T>>::insert(&task_hash, order_items);

        // issue events
        
        Ok(())
    }
    // 
    fn accept_simple_prefunded_closed_order(fullfiller: T::AccountId, ) -> Result {
        Ok(())
    }
    // Accepting the order means that it converts to a closed order for further processing
    fn accept_simple_prefunded_open_order() -> Result {
        Ok(())
    }
    fn complete_simple_prefunded_closed_order() -> Result {
        Ok(())
    }
    fn accept_prefunded_invoice() -> Result {
        Ok(())
    }
    //********************************************//
    //** Utilities *******************************//
    //********************************************//
    fn set_status_order() -> Result {
        Ok(())
    }
    fn get_pseudo_random_value(data: ) -> [u8; 16] {
        let input = (
            <timestamp::Module<T>>::get(),
            <system::Module<T>>::random_seed(),
            data,
            <system::Module<T>>::extrinsic_index(),
            <system::Module<T>>::block_number(),
        );
        return input.using_encoded(blake2_128);
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