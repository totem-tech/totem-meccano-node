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

//! # This is the Totem Orders Module 
//!
//! ## Overview
//!
//! The orders module supports creation of purchase orders and tasks and other types of market order.
//! A basic workflow is as follows:
//! * In general orders are assigned to a partner that the ordering identity already knows and is required to be accepted by that party to become active.
//! * Orders can be made without already knowing the seller - these are called market orders
//! * The order can be prefunded by calling into the prefunding module, which updates the accounting ledgers.
//! * Once the order is accepted, the work must begin, and once completed, the vendor sets the state to completed.
//! * The completion state also generates the invoice, and relevant accounting postings for both the buyer and the seller.
//! * The completed work is then approved by the buyer (or disputed or rejected). An approval triggers the release of prefunds and 
//! the invoice is marked as settled in the accounts for both parties
//! 
//! The main types used in this module are:
//!
//! * Product = Hash;
//! * UnitPrice = i128; // This does not need a unit of currency because it is allways the internal functional currency
//! * Quantity = u128;
//! * UnitOfMeasure = u16;
//! * buy_or_sell: u16, // 0: buy, 1: sell, extensible
//! * amount: AccountBalanceOf<T>, // amount should be the sum of all the items untiprices * quantities
//! * open_closed: bool, // 0: open(true) 1: closed(false)
//! * order_type: u16, // 0 Services, 1 Goods, 2 Inventory
//! * deadline: u64, // prefunding acceptance deadline 
//! * due_date: u64, // due date is the future delivery date (in blocks) 

use support::{
    decl_event, 
    decl_module, 
    decl_storage, 
    dispatch::Result, 
    StorageMap
};

use system::ensure_signed;
use parity_codec::{Decode, Encode};
use runtime_primitives::traits::{Convert};
use rstd::prelude::*;
// use node_primitives::Hash; // Use only in full node

// Totem Traits
use crate::accounting_traits::{ Posting };
use crate::prefunding_traits::{ Encumbrance };
use crate::bonsai_traits::{ Storing };
use crate::orders_traits::{ Validating };

// Totem Trait Types
type AccountBalanceOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::LedgerBalance;

// 0=Unlocked(false) 1=Locked(true)
pub type UnLocked<T> = <<T as Trait>::Prefunding as Encumbrance<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::UnLocked; 

// Substrate trait types

// Module Types
type OrderStatus = u16; // Generic Status for whatever the HashReference refers to
type ApprovalStatus = u16; // submitted(0), accepted(1), rejected(2)

// This is the order header: contains common values for all items
#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct OrderHeader<AccountId> {
    pub commander: AccountId,
    pub fulfiller: AccountId,
    pub approver: AccountId,
    pub order_status: u16,
    pub approval_status: u16,
    pub buy_or_sell: u16,
    pub amount: i128,
    pub open_closed: bool,
    pub order_type: u16,
    pub deadline: u64,
    pub due_date: u64,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct OrderItem<Hash> {
    pub product: Hash,
    pub unit_price: i128,
    pub quantity: u128,
    pub unit_of_measure: u16,
}

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Conversions: 
    Convert<i128, AccountBalanceOf<Self>> + 
    Convert<i128, u128> + 
    Convert<bool, UnLocked<Self>> + 
    Convert<AccountBalanceOf<Self>, i128> + 
    Convert<AccountBalanceOf<Self>, u128> + 
    Convert<u64, Self::BlockNumber> +
    Convert<Self::BlockNumber, u64>;
    type Accounting: Posting<Self::AccountId,Self::Hash,Self::BlockNumber>;
    type Prefunding: Encumbrance<Self::AccountId,Self::Hash,Self::BlockNumber>;
    type Bonsai: Storing<Self::Hash>;
}

decl_storage! {
    trait Store for Module<T: Trait> as OrdersModule {
        Owner get(owner): map T::AccountId => Vec<T::Hash>;
        Beneficiary get(beneficiary): map T::AccountId => Vec<T::Hash>;
        Approver get(approver): map T::AccountId => Vec<T::Hash>;
        Postulate get(postulate): map T::Hash => Vec<T::AccountId>;
        Orders get(orders): map T::Hash => Option<OrderHeader<T::AccountId>>;
        OrderItems get(order_items): map T::Hash => Vec<OrderItem<T::Hash>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// Complex Purchase Order
        fn create_po(
            origin,
            approver: T::AccountId, 
            fulfiller: T::AccountId, 
            buy_or_sell: u16, 
            total_amount: i128, 
            open_closed: bool, 
            order_type: u16, 
            deadline: u64, 
            due_date: u64, 
            order_items: Vec<OrderItem<T::Hash>>, 
            bonsai_token: T::Hash, 
            tx_uid: T::Hash
        ) -> Result {
            let _who = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            Ok(())
        }
        /// Create Simple Prefunded Service Order
        /// Can specify an approver. If the approver is the same as the sender then the order is considered approved by default
        fn create_spfso(
            origin,
            approver: T::AccountId, 
            fulfiller: T::AccountId, 
            buy_or_sell: u16, // 0: buy, 1: sell, extensible
            total_amount: i128, // amount should be the sum of all the items untiprices * quantities
            open_closed: bool, // 0: open(false) 1: closed(true)
            order_type: u16, // 0: service, 1: inventory, 2: asset extensible 
            deadline: u64, // prefunding acceptance deadline 
            due_date: u64, // due date is the future delivery date (in blocks) 
            order_item: OrderItem<T::Hash>, // for simple items there will only be one item, item number is accessed by its position in Vec 
            bonsai_token: T::Hash, // Bonsai data Hash
            tx_uid: T::Hash // Bonsai data Hash
        ) -> Result {
            let who = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid.clone())?;
            // Generate Hash for order
            let order_hash: T::Hash = <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber>>::get_pseudo_random_hash(who.clone(),approver.clone());
            
            if <Orders<T>>::exists(&order_hash) {
                Self::deposit_event(RawEvent::ErrorHashExists(order_hash));
                return Err("The hash already exists! Try again.");
            }
            
            Self::set_simple_prefunded_service_order(
                who,
                approver,
                fulfiller,
                buy_or_sell,
                total_amount,
                open_closed,
                order_type,
                deadline,
                due_date,
                order_hash,
                order_item,
                bonsai_token,
                tx_uid
            )?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            // issue events
            Self::deposit_event(RawEvent::OrderCreated(tx_uid, order_hash));
            Ok(())
        }
        /// Change Simple Prefunded Service Order.
        /// Can only be changed by the original ordering party, and only before it is accepted and the deadline or due date is not passed
        fn change_spfso(
            origin, 
            approver: T::AccountId, 
            fulfiller: T::AccountId, 
            amount: i128, 
            deadline: u64, 
            due_date: u64, 
            order_item: OrderItem<T::Hash>,
            record_id: T::Hash,
            bonsai_token: T::Hash, 
            tx_uid: T::Hash 
        ) -> Result {
            let who = ensure_signed(origin)?;
            // check owner of this record
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            Self::change_simple_prefunded_order(
                who.clone(), 
                approver.clone(),
                fulfiller.clone(),
                amount,
                deadline,
                due_date,
                order_item,
                record_id,
                bonsai_token
            )?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            // issue events
            Self::deposit_event(RawEvent::OrderUpdated(tx_uid));
            Ok(())
        }
        /// Sets the approval status of an order 
        /// Can only be used by the nominated approver (must be known to the ordering party)
        fn change_approval(origin, h: T::Hash, s: ApprovalStatus, b: T::Hash, tx_uid: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            Self::change_approval_state(who.clone(), h, s, b)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            Self::deposit_event(RawEvent::InvoiceSettled(h));
            Ok(())
        }
        
        fn handle_spfso_test(origin, h: T::Hash, s: OrderStatus, tx_uid: T::Hash) -> Result {
            let _who = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid.clone())?;
            // <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;            
            Ok(())
        }
        /// Can be used by buyer or seller
        /// Buyer - Used by the buyer to accept or reject (TODO) the invoice that was raised by the seller.
        /// Seller - Used to accept, reject or invoice the order. 
        fn handle_spfso(origin, h: T::Hash, s: OrderStatus, tx_uid: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid.clone())?;
            // get order details and determine if the sender is the buyer or the seller
            let order_hdr: OrderHeader<T::AccountId>;
            match Self::orders(&h) {
                Some(order) => order_hdr = order,
                None => {
                    Self::deposit_event(RawEvent::ErrorGettingOrder(tx_uid));
                    return Err("Unable to fetch order with this reference.");
                },
            };
            let commander: T::AccountId = order_hdr.commander.clone(); 
            let fulfiller: T::AccountId = order_hdr.fulfiller.clone();
            
            if who == commander {
                // This is the buyer 
                //TODO if the order us passed as an arg it doesn't need to be read again
                Self::accept_prefunded_invoice(who.clone(), h.clone(), s, order_hdr.clone(), tx_uid)?;
                Self::deposit_event(RawEvent::InvoiceSettled(tx_uid));
                
            } else if who == fulfiller {
                // This is the seller
                //TODO if the order us passed as an arg it doesn't need to be read again
                match Self::set_state_simple_prefunded_closed_order(who.clone(), h.clone(), s, order_hdr.clone(), tx_uid) {
                    Ok(_) => {
                        ();
                    },
                    Err(_e) => {
                        Self::deposit_event(RawEvent::ErrorSetPrefundState(tx_uid));
                        return Err("Error setting prefunding state");
                    },
                }
            } else {
                // this is an error
                Self::deposit_event(RawEvent::ErrorURNobody(tx_uid));
                return Err("You should not be doing this!");
                
            }
            
            <<T as Trait>::Bonsai as Storing<T::Hash>>::store_uuid(tx_uid)?;
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// The approver should be able to set the status, and once approved the process should continue further
    /// pending_approval (0), approved(1), rejected(2) are the tree states to be set
    /// If the status is 2 the commander may edit and resubmit
    fn check_approver(
        c: T::AccountId, 
        a: T::AccountId,
        h: T::Hash
    ) -> bool {
        // If the approver is the same as the commander then it is approved by default & update accordingly
        // If the approver is not the commander, then update but also set the status to pending approval. 
        // You should gracefully exit after this function call in this case.
        let mut approved: bool = false;
        if c == a { 
            approved = true; 
        };
        <Approver<T>>::mutate(&a, |approver| approver.push(h.clone()));
        
        approved
    }
    /// API Open an order for a specific AccountId and prefund it. This is equivalent to an encumbrance. 
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
        amount: i128, // amount should be the sum of all the items untiprices * quantities
        open_closed: bool, // 0: open(false) 1: closed(true)
        order_type: u16, // 0: personal, 1: business, extensible 
        deadline: u64, // prefunding acceptance deadline 
        due_date: u64, // due date is the future delivery date (in blocks) 
        order_hash: T::Hash,
        order_item: OrderItem<T::Hash>, // for simple items there will only be one item, item number is accessed by its position in Vec 
        bonsai_token: T::Hash,
        uid: T::Hash
    ) -> Result {
        
        // Set order status to submitted by default 
        // submitted(0), accepted(1), rejected(2), disputed(3), blocked(4), invoiced(5),
        let order_status: OrderStatus = 0;
        let mut fulfiller_override: T::AccountId = fulfiller.clone();
        let mut market_order: bool = false;
        match open_closed {
            true => {
                // this is a closed order, still will need to check or set the approver status
                // if fulfiller is the commander throw error
                if commander == fulfiller {
                    Self::deposit_event(RawEvent::ErrorCannotBeBoth(bonsai_token));
                    return Err("Cannot make an order for yourself!");
                }
            },
            // This is an open order. No need to check the fulfiller, but will override with the commander for time being.
            false => 
            {
                market_order = true;
                fulfiller_override = commander.clone();
            },
        }
        // check or set the approver status
        if Self::check_approver(commander.clone(), approver.clone(), order_hash.clone()) {
            // the order is approved.
            let approval_status: ApprovalStatus = 1;
            let deadline_converted: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(deadline.clone());
            // approval status has been set to approved, continue.
            
            // Set prefunding first. It does not matter if later the process fails, as this is locking funds for the commander
            // The risk is that they cannot get back the funds until after the deadline, even of they want to cancel.
            let balance_amount: u128 = <T::Conversions as Convert<i128, u128>>::convert(amount.clone());
            
            match Self::set_prefunding(commander.clone(), fulfiller.clone(), balance_amount, deadline_converted, order_hash.clone(), uid) {
                Ok(_) => (),
                Err(_e) => {
                    // Error from setting prefunding "somewhere" ;)
                    Self::deposit_event(RawEvent::ErrorInPrefunding1(uid));
                    return Err("Error in Prefunding Module");
                },
            }
            
            let order_header: OrderHeader<T::AccountId> = OrderHeader {
                commander: commander.clone(),
                fulfiller: fulfiller_override.clone(),
                approver: approver,
                order_status: order_status,
                approval_status: approval_status,
                buy_or_sell: buy_or_sell,
                amount: amount,
                open_closed: market_order,
                order_type: order_type,
                deadline: deadline,
                due_date: due_date,
            };
            
            let mut vec_order_items: Vec<OrderItem<T::Hash>> = Vec::new();
            vec_order_items.push(order_item.clone());
            
            Self::set_order(commander, fulfiller, order_hash.clone(), order_header, vec_order_items)?;
            
        } else {
            // the order is not yet approved.
            // This is NOT an error but requires further processing by the approver. Exiting gracefully.
            Self::deposit_event(RawEvent::OrderCreatedForApproval(bonsai_token.clone(), order_hash.clone()));
        }
        
        // claim hash in Bonsai
        <<T as Trait>::Bonsai as Storing<T::Hash>>::claim_data(order_hash.clone(), bonsai_token.clone())?;
        
        Ok(())
    }
    /// Calls the prefunding module to lock funds. This does not perform an update or lock release
    fn set_prefunding(
        c: T::AccountId, 
        f: T::AccountId, 
        a: u128, 
        d: T::BlockNumber,
        o: T::Hash,
        u: T::Hash
    ) -> Result {
        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::prefunding_for(c.clone(), f.clone(), a, d, o.clone(), u) {
            Ok(_) => (),
            Err(_e) => {
                Self::deposit_event(RawEvent::ErrorInPrefunding8(u));
                return Err("Error in prefunding");
            },
        }
        
        Ok(())
    }
    
    /// Stores the order data and sets the order status. 
    fn set_order(
        c: T::AccountId, 
        f: T::AccountId, 
        o: T::Hash,
        h: OrderHeader<T::AccountId>, 
        i: Vec<OrderItem<T::Hash>>
    ) -> Result {
        
        // Set hash for commander
        <Owner<T>>::mutate(&c, |owner| owner.push(o.clone()));
        
        // Set hash for fulfiller
        <Beneficiary<T>>::mutate(&f, |beneficiary| beneficiary.push(o.clone()));
        
        // Set details of Order
        <Orders<T>>::insert(&o, h);
        <OrderItems<T>>::insert(&o, i);
        
        Ok(())
    }
    /// API This function is used to accept or reject the order by the named approver. Mainly used for the API
    fn change_approval_state(a: T::AccountId, h: T::Hash, s: ApprovalStatus, b: T::Hash) -> Result {
        
        // is the supplied account the approver of the hash supplied?
        let mut order_hdr: OrderHeader<T::AccountId> = Self::orders(&h).ok_or("some error")?;
        
        if a == order_hdr.approver && order_hdr.order_status == 0 {
            match order_hdr.order_status {
                0 | 2 => {
                    // can only change to approved (1)
                    match s {
                        1 => (),
                        _ => {
                            // All other values not allowed
                            Self::deposit_event(RawEvent::ErrorApprStatus(h));
                            return Err("The submitted status not allowed.");
                        },
                    }
                },
                1 => {
                    // Can only change to 0 or 2
                    match s {
                        0 | 2 => (),
                        _ => {
                            // All other values not allowed
                            Self::deposit_event(RawEvent::ErrorApprStatus(h));
                            return Err("The submitted status not allowed.");
                        },
                    }
                },
                _ => {
                    // All other values not allowed
                    Self::deposit_event(RawEvent::ErrorApprStatus(h));
                    return Err("The submitted status not allowed.");
                }
            }
            
            // All tests passed, set status to whatever.
            order_hdr.order_status = s;
            
            <Orders<T>>::insert(&h, order_hdr);
            
        } else {
            Self::deposit_event(RawEvent::ErrorNotApprover(h));
            return Err("Cannot change an order that you are not the approver of");
        }
        
        Self::deposit_event(RawEvent::OrderStatusUpdate(b));
        
        Ok(())
        
    }
    /// API Allows commander to change the order either before it is accepted by beneficiary, or
    /// when it has been rejected by approver
    fn change_simple_prefunded_order(
        commander: T::AccountId, 
        approver: T::AccountId, 
        fulfiller: T::AccountId, 
        amount: i128, 
        deadline: u64, 
        due_date: u64, 
        order_item: OrderItem<T::Hash>,
        reference: T::Hash,
        bonsai_token: T::Hash
    ) -> Result {
        // Check that the hash exist
        // let order_hdr: OrderHeader<T::AccountId> = Self:order_header(&reference).ok_or("some error")?;
        let order_hdr: OrderHeader<T::AccountId> = Self::orders(&reference).ok_or("some error")?;
        
        // check that the Order state is 0 or 2 (submitted or rejected)
        // check that the approval is 0 or 2 pending approval or rejected
        match order_hdr.order_status {
            0 | 2 => {
                match order_hdr.approval_status {
                    0 | 2 => (), // submitted pending approval or rejected
                    1 => {
                        Self::deposit_event(RawEvent::ErrorApproved(reference));
                        return Err("Already approved!");
                    },
                    _ => {
                        Self::deposit_event(RawEvent::ErrorApprStatus(reference));
                        return Err("Incorrect Approval Status");
                    },
                };
            },
            1 => {
                Self::deposit_event(RawEvent::ErrorOrderStatus1(reference));
                return Err("Order already accepted - cannot change now!");
            },
            _ => {
                Self::deposit_event(RawEvent::ErrorOrderStatus2(reference));
                return Err("Incorrect Order Status!");
            },
        };
        
        // check that at least one of these has changed:
        // let mut dl: u64;
        // let mut dd: u64; 
        
        let current_block = <system::Module<T>>::block_number();
        
        // apply a new fulfiller but check that it isn't the commander
        if order_hdr.commander == commander {
            Self::deposit_event(RawEvent::ErrorFulfiller(reference));
            return Err("Not allowed to fulfill your own order!");
        }
        
        if order_hdr.amount != amount {
            if amount < 0i128 {
                Self::deposit_event(RawEvent::ErrorAmount(bonsai_token));
                return Err("Amount cannot be less than zero!");
            }
            
            // IMPORTANT TODO 
            // Check that the amount is the sum of all the items 
        }
        
        let current_block_converted: u64 = <T::Conversions as Convert<T::BlockNumber, u64>>::convert(current_block);
        if order_hdr.deadline != deadline {
            // TODO This may be unusable/unworkable needs trying out
            // 48 hours is the minimum deadline
            // every time there is a change the deadline gets pushed back by 48 hours byond the current block 
            let min_deadline: u64 = current_block_converted + 11520u64;
            if deadline < min_deadline {
                Self::deposit_event(RawEvent::ErrorShortDeadline(bonsai_token));
                return Err("Deadline is too short!");
            }
            // dl = deadline;
        }
        
        if order_hdr.due_date != due_date {
            // due date must be at least 1 hours after deadline (TODO - Validate! as this is a guess)
            // This is basically adding 49 hours to the current block
            let minimum_due_date: u64 = current_block_converted + 11760u64;
            if due_date < minimum_due_date {
                Self::deposit_event(RawEvent::ErrorShortDueDate(bonsai_token));
                return Err("Due Date is too short!");
            }
            // dd = due_date;
        }    
        // Create Order sub header
        let order_header: OrderHeader<T::AccountId> = OrderHeader {
            commander: commander.clone(),
            fulfiller: fulfiller.clone(),
            approver: approver.clone(),
            order_status: 0,
            approval_status: order_hdr.approval_status,
            buy_or_sell: order_hdr.buy_or_sell,
            amount: amount,
            open_closed: order_hdr.open_closed,
            order_type: order_hdr.order_type,
            deadline: deadline,
            due_date: due_date,
        };
        
        // currently just places all the items in the storage WITHOUT CHECKING
        // TODO check for changes and confirm that amount = sum of all amounts
        let mut vec_order_items: Vec<OrderItem<T::Hash>> = Vec::new();
        vec_order_items.push(order_item);
        
        Self::set_order(order_hdr.commander, fulfiller, reference.clone(), order_header, vec_order_items)?;
        
        // prefunding can only be cancelled if deadline has passed, otherwise the prefunding remains as a deposit
        // TODO we could use the cancel prefunding function to do this.
        
        // change hash in Bonsai
        <<T as Trait>::Bonsai as Storing<T::Hash>>::claim_data(reference.clone(), bonsai_token.clone())?;
        
        Ok(())
    }
    /// Used by the beneficiary (fulfiller) to accept, reject or invoice the order. 
    /// It effectively creates a state change for the order and the prefunding
    /// When accepting, the order is locked for the beneficiary or when rejected the funds are released for the order owner.
    /// When invoicing the 
    fn set_state_simple_prefunded_closed_order(f: T::AccountId, h: T::Hash, s: OrderStatus, mut order: OrderHeader<T::AccountId>, uid: T::Hash) -> Result {
        match order.order_status {
            0 => {
                // Order not accepted yet. Update the status in this module
                match s {
                    1 => {
                        // Order Accepted
                        // Update the prefunding status (confirm locked funds)
                        let lock: UnLocked<T> = <T::Conversions as Convert<bool, UnLocked<T>>>::convert(true);
                        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::set_release_state(f,lock,h,uid) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInPrefunding2(uid));
                                return Err("Error in prefunding");
                            },
                        }
                        
                    },
                    2 => {
                        // order rejected
                        let lock: UnLocked<T> = <T::Conversions as Convert<bool, UnLocked<T>>>::convert(false);
                        // set release state for releasing funds for fulfiller.
                        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::set_release_state(f,lock,h,uid.clone()) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInPrefunding3(uid));
                                return Err("Error in prefunding");
                            },
                        }
                        
                        // set release state for releasing funds for commander.
                        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::set_release_state(order.commander.clone(),lock,h,uid.clone()) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInPrefunding4(uid));
                                return Err("Error in prefunding");
                            },
                        }
                        
                        // now release the funds lock
                        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::unlock_funds_for_owner(order.commander.clone(),h, uid.clone()) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInPrefunding5(uid));
                                return Err("Error in prefunding");
                            },
                        }
                        
                    },
                    _ => {
                        Self::deposit_event(RawEvent::ErrorStatusNotAllowed1(uid));
                        return Err("Order status is not allowed!");
                    },
                }
            },
            1 => {
                // Order already in accepted state - Update the status
                match s {
                    5 => {
                        // Order Completed. Now we are going to issue the invoice.
                        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::send_simple_invoice(f.clone(), order.commander.clone(), order.amount, h, uid) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInPrefunding6(uid));
                                return Err("Error in prefunding");
                            },
                        }
                        
                    },
                    _ => {
                        Self::deposit_event(RawEvent::ErrorStatusNotAllowed2(uid));
                        return Err("Order status is not allowed!");
                    },
                }
                
            },
            2 | 5  => {
                Self::deposit_event(RawEvent::ErrorStatusNotAllowed3(uid));
                return Err("The order has a status that cannot be changed!");
            },
            _ => {
                Self::deposit_event(RawEvent::ErrorStatusNotAllowed4(uid));
                return Err("The order has an unkown state!");
            },
        }
        order.order_status = s;
        
        <Orders<T>>::remove(&h);
        <Orders<T>>::insert(&h, order);
        
        Self::deposit_event(RawEvent::OrderCompleted(uid));
        Ok(())
    }
    /// Used by the buyer to accept or reject (TODO) the invoice that was raised by the seller.
    fn accept_prefunded_invoice(o: T::AccountId, h: T::Hash, s: OrderStatus, mut order: OrderHeader<T::AccountId>, uid: T::Hash) -> Result {
        // check that this is the fulfiller
        match order.order_status {
            5 => {
                // Order has been invoiced. The buyer is now deciding to accept or other
                match s {
                    3 => {
                        // Invoice is disputed. TODO provide the ability to change the invoice and resubmit
                        Self::deposit_event(RawEvent::ErrorNotImplmented1(uid));
                        
                        return Err("TODO!");
                    },
                    6 => {
                        // Invoice Accepted. Now pay-up!.
                        match <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::settle_prefunded_invoice(o.clone(), h, uid) {
                            Ok(_) => (),
                            Err(_e) => {
                                Self::deposit_event(RawEvent::ErrorInPrefunding7(uid));
                                return Err("Error in prefunding");
                            },
                        }
                        
                        Self::deposit_event(RawEvent::InvoiceSettled(uid));
                    },
                    _ => {
                        // All other states are not allowed
                        Self::deposit_event(RawEvent::ErrorStatusNotAllowed5(uid));
                        return Err("The order has an unkown state!");
                    },
                }
                // Update the status in this module
            },
            _ => {
                Self::deposit_event(RawEvent::ErrorOrderStatus3(uid));
                return Err("The order has an unkown state!");
            },
        }
        order.order_status = s;
        <Orders<T>>::remove(&h);
        <Orders<T>>::insert(&h, order);
        
        Ok(())
    }
    /// This is used by any party that wants to accept a market order in whole or part. 
    /// This is non-blocking and can accept many applicants
    fn postulate_simple_prefunded_open_order() -> Result {
        Ok(())
    }
}

impl<T: Trait> Validating<T::AccountId, T::Hash> for Module<T> {
    /// Check that the order is somehow managed by this identity. Mainly used for BONSAI
    fn is_order_party(o: T::AccountId, r: T::Hash) -> bool {
        let mut answer: bool = false;
        
        match Self::orders(r) {
            Some(order) => {
                let commander = order.commander.clone();
                let fulfiller = order.fulfiller.clone();
                let approver = order.approver.clone();
                if o == commander || o == fulfiller || o == approver {
                    answer = true;
                };
            },
            None => (), // error - return false
        }
        
        answer
    }
}

decl_event!(
    pub enum Event<T> where
    Hash = <T as system::Trait>::Hash,
    {
        OrderCreated(Hash, Hash),
        OrderUpdated(Hash),
        OrderCreatedForApproval(Hash, Hash),
        OrderStatusUpdate(Hash),
        OrderCompleted(Hash),
        InvoiceSettled(Hash),
        /// Cannot change an order that you are not the approver of
        ErrorNotApprover(Hash),
        /// This hash already exists! Try again.
        ErrorHashExists(Hash),
        /// Cannot make an order for yourself!
        ErrorCannotBeBoth(Hash),
        /// You should not be doing this!
        ErrorURNobody(Hash),
        /// Order already accepted - cannot change now!
        ErrorOrderStatus1(Hash),
        /// Incorrect Order Status!
        ErrorOrderStatus2(Hash),
        /// The order has an unkown state!
        ErrorOrderStatus3(Hash),
        /// The submitted status not allowed.
        ErrorApprStatus(Hash),
        /// Already approved!
        ErrorApproved(Hash),
        /// Order status is not allowed!
        ErrorStatusNotAllowed1(Hash),
        /// Order already accepted. Order status is not allowed!
        ErrorStatusNotAllowed2(Hash),
        /// The order has a status that cannot be changed!
        ErrorStatusNotAllowed3(Hash),
        /// The order has an unkown state!
        ErrorStatusNotAllowed4(Hash),
        /// The order has an unkown state!
        ErrorStatusNotAllowed5(Hash),
        /// Not allowed to fulfill your own order!
        ErrorFulfiller(Hash),
        /// Amount cannot be less than zero!
        ErrorAmount(Hash),
        /// Deadline is too short! 48 hours is minimum deadline.
        ErrorShortDeadline(Hash),
        /// Due date must be at least 1 hour after deadline
        ErrorShortDueDate(Hash),
        /// This situation is not implemented yet: Invoice is disputed
        ErrorNotImplmented1(Hash),
        /// Unable to fetch order with this reference
        ErrorGettingOrder(Hash),
        /// Error setting prefunding state
        ErrorSetPrefundState(Hash),
        /// Error from prefunding module - in check approver
        ErrorInPrefunding1(Hash),
        /// Error in Processing Order Acceptance status 
        ErrorInPrefunding2(Hash),
        /// Error in rejecting order setting release state for fulfiller
        ErrorInPrefunding3(Hash),
        /// Error in rejecting order adjusting commander settings
        ErrorInPrefunding4(Hash),
        /// Error in rejecting order releasing commander lock
        ErrorInPrefunding5(Hash),
        /// Error in prefunding module to send invoice
        ErrorInPrefunding6(Hash),
        /// Error in prefunding settling invoice
        ErrorInPrefunding7(Hash),
        /// Error setting the first prefunding request
        ErrorInPrefunding8(Hash),
    }
);