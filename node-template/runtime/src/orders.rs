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

// use system::ensure_signed;
use parity_codec::{Decode, Encode};
use runtime_primitives::traits::{Convert};
use rstd::prelude::*;
// use node_primitives::Hash; // Use only in full node
use primitives::H256;

// Totem Traits
use crate::accounting_traits::{ Posting };
use crate::prefunding_traits::{ Encumbrance };

// Totem Trait Types
type AccountOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::Account;
type AccountBalanceOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::AccountBalance;

// Other trait types
// type CurrencyBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

// Module Types
type OrderStatus = u16; // Generic Status for whatever the HashReference refers to
type ApprovalStatus = u16; // submitted(0), accepted(1), rejected(2)

type Product = H256; // `Hash` in full node
type UnitPrice = i128; 
type Quantity = i128;
type OrderHeader = (u16, T::AccounBalanceOf, bool, u16, u64, u64);
type OrderItem = Vec<(Product, UnitPrice, Quantity)>;


pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Conversions: Convert<i128, AccountBalanceOf<Self>> + Convert<AccountBalanceOf<Self>, i128>;
    type Accounting: Posting<Self::AccountId,Self::Hash,Self::BlockNumber>;
    type Prefunding: Encumbrance<Self::AccountId,Self::Hash,Self::BlockNumber>;
}

decl_storage! {
    trait Store for Module<T: Trait> as OrdersModule {
        Owner get(owner): map T::AccountId => Vec<T::Hash>;
        Beneficiary get(beneficiary): map T::AccountId => Vec<T::Hash>;
        Approver get(approver): map T::AccountId => Vec<T::Hash>;
        Header get(header): map T::Hash => Option<(bool, <AccountBalanceOf<T>, bool, u16, T::BlockNumber, T::BlockNumber)>;
        Details get(details): map T::Hash => OrderItem;
        Status get(status): map T::Hash => Option<OrderStatus>;
        Approved get(approved): map T::Hash => Option<ApprovalStatus>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}


// {
//     buy: integer (0: sell, 1: buy),
//     assignee: identity,
//     currency: string (XTX, USD....),
//     description: string  
//     reward: integer
//     title: string
//     status: integer (same as timekeeping, and maybe some more task-specific statuses)
//     type:  integer (1: business, 0: personal) (same as with the identity derivation path) 
//   }
//   token: hash

impl<T: Trait> Module<T> {
    // The approver should be able to set the status, and once approved the process should continue further
    // pending_approval (0), accepted(1), rejected(2) are the tree states to be set
    // If the status is 2 the commander may edit and resubmit
    fn initial_approval_state(c: T::AccountId, 
            a: T::AccountId,
            h: T::Hash
    ) -> bool {
        // If the approver is the same as the commander then it is approved by default & update accordingly
        // If the approver is not the commander, then update but also set the status to pending approval. 
        // You should gracefully exit after this function call in this case.
        let mut status: ApprovalStatus = 0;
        if c == a { status = 1 };
        <Approver<T>>::mutate(&a, |approver| approver.push(h.clone()));
        <Approved<T>>::insert(h, status);
        return true;
    } 
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
        buy_or_sell: u16, // 0: buy, 1: sell, extensible
        amount: AccountBalanceOf<T>, // amount should be the sum of all the items untiprices * quantities
        open_closed: bool, // 0: open(true) 1: closed(false)
        order_type: u16, // 0: personal, 1: business, extensible 
        deadline: T::BlockNumber, // prefunding acceptance deadline 
        due_date: T::BlockNumber, // due date is the future delivery date (in blocks) 
        // order_items: Vec<(Product, UnitPrice, Quantity)> // for simple items there will only be one item, item number is accessed by its position in Vec 
        order_items: OrderItem // for simple items there will only be one item, item number is accessed by its position in Vec 
    ) -> Result {
        
        // Generate Hash for order
        let order_hash = <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber>>::get_pseudo_random_hash(commander.clone(),approver.clone());
        // TODO check that it does not already exist
        ensure!(!<Status<T>>::exists(&order_hash), "The hash already exists! Try again.");
        
        if open_closed {
            // This is an open order. No need to check the fulfiller, but will need to check or set the approver status
            ();
        } else {
            // this is a closed order, still will need to check or set the approver status
            // check that the fulfiller is not the commander as this makes no sense
            if !open_closed && commander == approver {
                return Err("Cannot make an order for yourself!");
            }
        }
        // check or set the approver status
        if Self::initial_approval_state(commander.clone(), approver.clone(), order_hash) {
            // approval status has been set to approved, continue.
            // let prefund_amount: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount);
            // let order_header = (bool, <AccountBalanceOf<T>, bool, u16, T::BlockNumber, T::BlockNumber);
            let order_header = (buy_or_sell, amount, open_closed, order_type, deadline, due_date);
            Self::record_approved_order(commander.clone(), fulfiller.clone(), order_hash, order_header, order_items)?;

        } else {
            // This is not an error but requires further processing by the approver. Exiting gracefully.
            ();
        }

        Ok(())
    }
    
    fn record_approved_order(c: T::AccountId, f: T::AccountId, o: T::Hash, h: OrderHeader, i: OrderItem ) -> Result {
        
        // Set Prefunding - do this now, it does not matter if there are errors after this point.
        ensure!(c != f, "Beneficiary must be another account");
        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::prefunding_for(c.clone(), f.clone(), h.1, h.5)?;
        // Set order status to submitted 
        let status: OrderStatus = 0;
        
        // Set hash for commander
        <Owner<T>>::mutate(&c, |owner| owner.push(o.clone()));
        
        // Set Acceptance Status
        // submitted(0), accepted(1), rejected(2), disputed(3), blocked(4), invoiced(5), reason_code(0), reason text.
        <Status<T>>::insert(&o, status);
        
        
        // Set hash for fulfiller
        <Beneficiary<T>>::mutate(&f, |b| b.push(o.clone()));

        // Set details of Order
        <Header<T>>::insert(&o, h);
        <Details<T>>::insert(&o, i);

        // issue events
        Self::deposit_event(RawEvent::OrderCreated(c, f, o));
        
        Ok(())
    }

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
}

decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    Hash = <T as system::Trait>::Hash
    {
        OrderCreated(AccountId, AccountId, Hash),
    }
);