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
// * In general orders are assigned to a partner that the ordering identity already knows and is required to be accepted by that party to become active.
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
use runtime_primitives::traits::{Convert};
use rstd::prelude::*;
use node_primitives::Hash; // Use only in full node
// use primitives::H256;

// Totem Traits
use crate::accounting_traits::{ Posting };
use crate::prefunding_traits::{ Encumbrance };

// Totem Trait Types
type AccountOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::Account;
type AccountBalanceOf<T> = <<T as Trait>::Accounting as Posting<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::AccountBalance;

// 0=Unlocked(false) 1=Locked(true)
pub type UnLocked<T> = <<T as Trait>::Prefunding as Encumbrance<<T as system::Trait>::AccountId,<T as system::Trait>::Hash,<T as system::Trait>::BlockNumber>>::UnLocked; 

// Other trait types
// type CurrencyBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

// Module Types
type OrderStatus = u16; // Generic Status for whatever the HashReference refers to
type ApprovalStatus = u16; // submitted(0), accepted(1), rejected(2)

type Product = Hash;
type UnitPrice = i128; 
type Quantity = u128;
type UnitOfMeasure = u16;

// buy_or_sell: u16, // 0: buy, 1: sell, extensible
// amount: AccountBalanceOf<T>, // amount should be the sum of all the items untiprices * quantities
// open_closed: bool, // 0: open(true) 1: closed(false)
// order_type: u16, // 0 Services, 1 Goods, 2 Inventory
// deadline: u64, // prefunding acceptance deadline 
// due_date: u64, // due date is the future delivery date (in blocks) 
type OrderSubHeader = (u16, i128, bool, u16, u64, u64);

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ItemDetailsStruct(Product, UnitPrice, Quantity, UnitOfMeasure);

// type OrderItem = Vec<(Product, UnitPrice, Quantity, UnitOfMeasure)>;
// type OrderItem = Vec<ItemDetailsStruct>;
type OrderItem = ItemDetailsStruct;


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
}

decl_storage! {
    trait Store for Module<T: Trait> as OrdersModule {
        Owner get(owner): map T::AccountId => Vec<T::Hash>;
        Beneficiary get(beneficiary): map T::AccountId => Vec<T::Hash>;
        Approver get(approver): map T::AccountId => Vec<T::Hash>;
        Postulate get(postulate): map T::Hash => Vec<T::AccountId>;
        
        Order get(order): map T::Hash => Option<(T::AccountId,T::AccountId,T::AccountId,u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber)>;
        Details get(details): map T::Hash => OrderItem; // Could be Vec here
        Status get(status): map T::Hash => Option<OrderStatus>;
        Approved get(approved): map T::Hash => Option<ApprovalStatus>;
        // Order get(order): map T::Hash => Option<(bool,u16)>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        /// Create Simple Prefunded Service Order
        /// Can specify an approver. If the approver is the sale as the sender then the order is considered approved by default
        fn create_spfso(
            origin,
            approver: T::AccountId, 
            fulfiller: T::AccountId, 
            buy_or_sell: u16, // 0: buy, 1: sell, extensible
            total_amount: i128, // amount should be the sum of all the items untiprices * quantities
            open_closed: bool, // 0: open(true) 1: closed(false)
            order_type: u16, // 0: service, 1: inventory, 2: asset extensible 
            deadline: u64, // prefunding acceptance deadline 
            due_date: u64, // due date is the future delivery date (in blocks) 
            order_items: OrderItem // for simple items there will only be one item, item number is accessed by its position in Vec 
        ) -> Result {
            let who = ensure_signed(origin)?;
            let amount: AccountBalanceOf<T> = <T::Conversions as Convert<i128, AccountBalanceOf<T>>>::convert(total_amount);
            Self::set_simple_prefunded_service_order(
                who.clone(),
                approver.clone(),
                fulfiller.clone(),
                buy_or_sell,
                amount,
                open_closed,
                order_type,
                deadline,
                due_date,
                order_items
            )?;
            Ok(())
        }
        fn test_vec_tuple(origin, _order_items: Vec<ItemDetailsStruct>) -> Result {
            let _ = ensure_signed(origin)?;
            // Self::deposit_event(RawEvent::Test(_order_items));
            Ok(())
        }
        /// Change Simple Prefunded Service Order.
        /// Can only be changed by the original ordering party, and only before it is accepted and the deadline or due date is not passed
        fn change_spfso(
            origin, 
            approver: T::AccountId, 
            fulfiller: T::AccountId, 
            amount: AccountBalanceOf<T>, 
            deadline: u64, 
            due_date: u64, 
            order_items: OrderItem,
            reference: T::Hash) -> Result {
                let who = ensure_signed(origin)?;
                Self::change_simple_prefunded_order(
                    who.clone(), 
                    approver.clone(),
                    fulfiller.clone(),
                    amount,
                    deadline,
                    due_date,
                    order_items,
                    reference)?;
                    Ok(())
                }
                /// Sets the approval status of an order 
                /// Can only be used by the nominated approver (must be known to the ordering party)
                fn change_approval(origin, h: T::Hash, s: ApprovalStatus) -> Result {
                    let who = ensure_signed(origin)?;
                    Self::change_approval_state(who.clone(), h, s)?;
                    Self::deposit_event(RawEvent::InvoiceSettled(h));

                    Ok(())
                }
                /// Can be used by buyer or seller
                /// Buyer - Used by the buyer to accept or reject (TODO) the invoice that was raised by the seller.
                /// Seller - Used to accept, reject or invoice the order. 
                fn handle_spfso(origin, h: T::Hash, s: OrderStatus) -> Result {
                    let who = ensure_signed(origin)?;
                    // get order details and determine if the sender is the buyer or the seller
                    match Self::order(&h) {
                        Some(order) => {
                            if who == order.0 {
                                // This is the buyer 
                                //TODO if the order us passed as an arg it doesn't need to be read again
                                Self::accept_prefunded_invoice(who.clone(), h, s)?;
                                Self::deposit_event(RawEvent::InvoiceSettled(h));
                                
                            } else if who == order.1 {
                                // This is the seller
                                //TODO if the order us passed as an arg it doesn't need to be read again
                                Self::set_state_simple_prefunded_closed_order(who.clone(), h, s)?;
                                
                            } else {
                                // this is an error
                                Self::deposit_event(RawEvent::ErrorURNobody(who));
                                return Err("You should not be doing this!");
                            }
                        },
                        None => {
                            Self::deposit_event(RawEvent::ErrorRefNotFound(h));
                            return Err("Could not find this reference!");
                        },
                    }
                    Ok(())
                }
            }
        }
        
        impl<T: Trait> Module<T> {
            /// The approver should be able to set the status, and once approved the process should continue further
            /// pending_approval (0), approved(1), rejected(2) are the tree states to be set
            /// If the status is 2 the commander may edit and resubmit
            fn set_init_appr_state(
                c: T::AccountId, 
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
                
                true
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
                amount: AccountBalanceOf<T>, // amount should be the sum of all the items untiprices * quantities
                open_closed: bool, // 0: open(true) 1: closed(false)
                order_type: u16, // 0: personal, 1: business, extensible 
                deadline: u64, // prefunding acceptance deadline 
                due_date: u64, // due date is the future delivery date (in blocks) 
                order_items: OrderItem // for simple items there will only be one item, item number is accessed by its position in Vec 
            ) -> Result {
                
                // Generate Hash for order
                let order_hash = <<T as Trait>::Accounting as Posting<T::AccountId,T::Hash,T::BlockNumber>>::get_pseudo_random_hash(commander.clone(),approver.clone());
                
                if <Status<T>>::exists(&order_hash) {
                    Self::deposit_event(RawEvent::ErrorHashExists(order_hash));
                    return Err("The hash already exists! Try again.");
                }
                // ensure!(!<Status<T>>::exists(&order_hash), "The hash already exists! Try again.");
                
                match open_closed {
                    true => (), // This is an open order. No need to check the fulfiller, but will need to check or set the approver status
                    false => {
                        if commander == fulfiller {
                            // this is a closed order, still will need to check or set the approver status
                            // check that the fulfiller is not the commander as this makes no sense
                            Self::deposit_event(RawEvent::ErrorCannotBeBoth(commander));
                            return Err("Cannot make an order for yourself!");
                        }
                    },
                }
                // check or set the approver status
                if Self::set_init_appr_state(commander.clone(), approver.clone(), order_hash) {
                    let deadline_converted: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(deadline);
                    let due_date_converted: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(due_date);
                    // approval status has been set to approved, continue.
                    
                    // Set prefunding first. It does not matter if later the process fails, as this is locking funds for the commander
                    // The risk is that they cannot get back the funds until after the deadline, even of they want to cancel.
                    Self:: set_prefunding(commander.clone(), fulfiller.clone(), amount, deadline_converted)?;
                    
                    // Set order status to submitted 
                    // submitted(0), accepted(1), rejected(2), disputed(3), blocked(4), invoiced(5), reason_code(0), reason text.
                    let status: OrderStatus = 0;
                    let balance_amount: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount);
                    
                    let order_header = (buy_or_sell, balance_amount, open_closed, order_type, deadline, due_date);
                    
                    Self::set_order_approval(commander.clone(), fulfiller.clone(), approver.clone(), order_hash, order_header, order_items, status)?;
                    
                    
                } else {
                    // This is NOT an error but requires further processing by the approver. Exiting gracefully.
                    Self::deposit_event(RawEvent::OrderCreatedForApproval(commander, approver, order_hash));
                }
                
                Ok(())
            }
            /// Calls the prefunding module to lock funds. This does not perform an update or lock release
            fn set_prefunding(
                c: T::AccountId, 
                f: T::AccountId, 
                a: AccountBalanceOf<T>, 
                d: T::BlockNumber
            ) -> Result {
                let a_converted: u128 = <T::Conversions as Convert<AccountBalanceOf<T>, u128>>::convert(a);
                <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::prefunding_for(c.clone(), f.clone(), a_converted, d)?;
                Ok(())
            }
            /// Utility to convert the OrderSubheader to a tuple used to update the storage. Returns a tuple  
            /// This is currently necessary because we use a tuple type for convenience which cannot use the AccountBalanceOf<T> type or T::BlockNumber in its definition.
            /// I'm still leanrning!!
            fn format_order_hdr(
                c: T::AccountId, 
                f: T::AccountId, 
                a: T::AccountId, 
                h: OrderSubHeader
            ) -> (T::AccountId,T::AccountId,T::AccountId,u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber) {
                let balance_amount: AccountBalanceOf<T> = <T::Conversions as Convert<i128, AccountBalanceOf<T>>>::convert(h.1);
                let deadline: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(h.4);
                let due_date: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(h.5);
                
                let order_hdr: (T::AccountId,T::AccountId,T::AccountId,u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber) = (c.clone(), f.clone(), a.clone(), h.0, balance_amount, h.2, h.3, deadline, due_date);
                
                order_hdr
                
            }
            /// Stores the order data and sets the order status. Takes tuples for OrderSubHeader and OrderItems
            fn set_order_approval(
                c: T::AccountId, 
                f: T::AccountId, 
                a: T::AccountId, 
                o: T::Hash, 
                h: OrderSubHeader, 
                i: OrderItem, 
                s: OrderStatus
            ) -> Result {
                
                let order_hdr: (T::AccountId,T::AccountId,T::AccountId,u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber) = Self::format_order_hdr(c.clone(), f.clone(), a.clone(), h); 
                
                // Set hash for commander
                <Owner<T>>::mutate(&c, |owner| owner.push(o.clone()));
                
                // Set Acceptance Status
                <Status<T>>::insert(&o, s);
                
                // Set hash for fulfiller
                <Beneficiary<T>>::mutate(&f, |b| b.push(o.clone()));
                
                // Set details of Order
                <Order<T>>::insert(&o, order_hdr);
                <Details<T>>::insert(&o, i);
                
                // TODO set the approval status!
                // issue events
                Self::deposit_event(RawEvent::OrderCreated(c, f, o));
                
                Ok(())
            }
            /// API This function is used to accept or reject the order by the named approver. Mainly used for the API
            fn change_approval_state(a: T::AccountId, h: T::Hash, s: ApprovalStatus) -> Result {
                
                // is the supplied account the approver of the hash supplied?
                match Self::order(&h) {
                    Some(order) => {
                        if a == order.2 {
                            // check the status being proposed
                            match s {
                                1 => {
                                    // as we are changing the status to approved, we need to record the 
                                    // Get OrderItems // Vec<(Product, UnitPrice, Quantity)>;
                                    let details = Self::details(&h);
                                    
                                    // Create Order sub header
                                    let converted_amount: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(order.4);
                                    let converted_deadline: u64 = <T::Conversions as Convert<T::BlockNumber, u64>>::convert(order.7);
                                    let converted_due_date: u64 = <T::Conversions as Convert<T::BlockNumber, u64>>::convert(order.8);
                                    // order_sub_hdr: (u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber)
                                    let sub:OrderSubHeader = (order.3, converted_amount, order.5, order.6, converted_deadline, converted_due_date); 
                                    let status: OrderStatus = 0;
                                    // (c: T::AccountId, f: T::AccountId, a: T::AccountId, o: T::Hash, h: OrderSubHeader, i: OrderItem )
                                    Self::set_order_approval(order.0, order.1, order.2, h, sub, details, status)?;
                                } 
                                2 => (), // rejected!
                                _ => {
                                    // not in scope
                                    Self::deposit_event(RawEvent::ErrorApprStatus(h));
                                    return Err("The submitted status not allowed.");
                                    
                                }, 
                            }
                        } else {
                            Self::deposit_event(RawEvent::ErrorNotApprover(a,h));
                            return Err("Cannot change an order that you are not the approver of");
                        }
                    },
                    None => {
                        Self::deposit_event(RawEvent::ErrorRefNotFound(h));
                        return Err("reference hash does not exist");
                    },
                }
                
                // if all is approved
                <Status<T>>::remove(&h);
                <Status<T>>::insert(&h, s);
                
                Self::deposit_event(RawEvent::OrderStatusUpdate(h, s));
                
                Ok(())
                
            }
            /// API Allows commander to change the order either before it is accepted by beneficiary, or
            /// when it has been rejected by approver
            fn change_simple_prefunded_order(
                commander: T::AccountId, 
                approver: T::AccountId, 
                fulfiller: T::AccountId, 
                amount: AccountBalanceOf<T>, 
                deadline: u64, 
                due_date: u64, 
                order_items: OrderItem,
                reference: T::Hash
            ) -> Result {
                // Check that the hash exist
                // check that the Order state is 0 or 2 (submitted or rejected)
                // check that the approval is 0 or 2 pending approval or rejected
                match Self::status(&reference) {
                    Some(0) | Some(2)  => {
                        match Self::approved(&reference) {
                            Some(0) | Some(2) => (),
                            Some(1) => {
                                Self::deposit_event(RawEvent::ErrorApproved(reference));
                                return Err("Already approved!");
                            },
                            None => {
                                Self::deposit_event(RawEvent::ErrorApprStatus(reference));
                                return Err("Approval status not found!");
                            },
                            _ => {
                                Self::deposit_event(RawEvent::ErrorApprStatus(reference));
                                return Err("Incorrect Approval Status");
                            },
                        }
                    },
                    Some(1) => {
                        Self::deposit_event(RawEvent::ErrorOrderStatus(reference));
                        return Err("Order already accepted - cannot change now!");
                    },
                    None => {
                        Self::deposit_event(RawEvent::ErrorRefNotFound(reference));
                        return Err("Hash not found!");
                    },
                    _ => {
                        Self::deposit_event(RawEvent::ErrorOrderStatus(reference));
                        return Err("Incorrect Order Status!");
                    },
                }
                // update header and items
                // Get the original order
                match Self::order(&reference) {
                    Some(order) => {
                        // check that at least one of these has changed:
                        let mut f: T::AccountId = order.1.clone();
                        let mut a: AccountBalanceOf<T> = order.4;
                        let mut dl: u64 = <T::Conversions as Convert<T::BlockNumber, u64>>::convert(order.7);
                        let mut dd: u64 = <T::Conversions as Convert<T::BlockNumber, u64>>::convert(order.8);
                        
                        let current_block = <system::Module<T>>::block_number();
                        
                        // apply a new fulfiller but check that it isn't the commander
                        match order.1.clone() {
                            fulfiller => (),
                            commander => {
                                Self::deposit_event(RawEvent::ErrorFulfiller(reference));
                                return Err("Not allowed to fulfill your own order!");
                            },
                            _ => f = fulfiller,
                        }
                        // apply a new amount
                        // This also indicates a change to the items
                        // TODO this should be replaced with a more thorough sanity check of the item changes                        
                        match order.4 {
                            amount => (),
                            _ => {
                                let amount_converted: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount);
                                // check that the amount is greater than zero
                                if amount_converted > 0i128 {
                                    a = amount;
                                } else {
                                    Self::deposit_event(RawEvent::ErrorAmount(amount));
                                    return Err("Amount cannot be less than zero!");
                                }
                            }
                        }
                        let current_block_converted: u64 = <T::Conversions as Convert<T::BlockNumber, u64>>::convert(current_block);
                        let deadline_converted: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(deadline);
                        match order.7 {
                            deadline_converted => (),
                            _ => {
                                // NEED TO CHECK THAT THE DEADLINE IS SENSIBLE!!!!
                                // 48 hours is the minimum deadline 
                                let min_deadline: u64 = current_block_converted + 11520u64;
                                
                                if deadline < min_deadline {
                                    Self::deposit_event(RawEvent::ErrorShortDeadline(current_block, deadline_converted));
                                    return Err("Deadline is too short!");
                                }
                                dl = deadline;
                            }
                        }
                        
                        let due_date_converted: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(due_date);
                        match order.8 {
                            due_date_converted => (),
                            _ => {
                                let minimum_due_date: u64 = current_block_converted + 11760u64;
                                // due date must be at least 1 hours after deadline (TODO - Validate! as this is a guess)
                                if due_date < minimum_due_date {
                                    Self::deposit_event(RawEvent::ErrorShortDueDate(current_block, due_date_converted));
                                    return Err("Due Date is too short!");
                                }
                                dd = due_date;
                            }
                        }
                        
                        // Create Order sub header
                        // order_sub_hdr: (buy_or_sell, converted_amount, open_closed, order_type, deadline, due_date)
                        let converted_amount: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount);
                        let sub:OrderSubHeader = (order.3, converted_amount, order.5, order.6, dl, dd); 
                        let status: OrderStatus = 0;
                        
                        Self::set_order_approval(order.0, order.1, order.2, reference, sub, order_items, status)?;
                        
                        // prefunding can only be cancelled if deadline has passed, otherwise the prefunding remains as a deposit
                        // TODO we could use the cancel prefunding function to do this, but also we need to check exchange rates
                        
                    },
                    None => return Err("Error getting order details"),
                }
                
                Ok(())
            }
            /// Used by the beneficiary (fulfiller) to accept, reject or invoice the order. 
            /// It effectively creates a state change for the order and the prefunding
            /// When accepting, the order is locked for the beneficiary or when rejected the funds are released for the order owner.
            /// When invoicing the 
            fn set_state_simple_prefunded_closed_order(f: T::AccountId, h: T::Hash, s: OrderStatus) -> Result {
                // check that this is the fulfiller
                match Self::order(&h) {
                    Some(order) => {
                        // check the currenct status
                        match Self::status(&h).unwrap() {
                            0 => {
                                // Update the status in this module
                                match s {
                                    1 => {
                                        // Order Accepted
                                        // Update the prefunding status (confirm locked funds)
                                        let lock: UnLocked<T> = <T::Conversions as Convert<bool, UnLocked<T>>>::convert(true);
                                        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::set_release_state(f,lock,h,false)?;
                                    },
                                    2 => {
                                        // order rejected
                                        let lock: UnLocked<T> = <T::Conversions as Convert<bool, UnLocked<T>>>::convert(false);
                                        // set release state for releasing funds for fulfiller.
                                        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::set_release_state(f,lock,h,false)?;
                                        // set release state for releasing funds for commander.
                                        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::set_release_state(order.0.clone(),lock,h,true)?;
                                        // now release the funds lock
                                        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::unlock_funds_for_owner(order.0,h)?;
                                    },
                                    _ => {
                                        Self::deposit_event(RawEvent::ErrorStatusNotAllowed(h,s));
                                        return Err("Order status is not allowed!");
                                    },
                                }
                            },
                            1 => {
                                // Update the status in this module
                                match s {
                                    5 => {
                                        // Order Completed. Now we are going to issue the invoice.
                                        let amount: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(order.4);
                                        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::send_simple_invoice(f.clone(), order.0.clone(), amount, h)?;
                                    },
                                    _ => {
                                        Self::deposit_event(RawEvent::ErrorStatusNotAllowed(h,s));
                                        return Err("Order status is not allowed!");
                                    },
                                }
                                
                            },
                            2 | 5  => {
                                Self::deposit_event(RawEvent::ErrorStatusNotAllowed(h, s));
                                return Err("The order has a status that cannot be changed!");
                            },
                            _ => {
                                Self::deposit_event(RawEvent::ErrorStatusNotAllowed(h, s));
                                return Err("The order has an unkown state!");
                            },
                        }
                    },
                    None => {
                        Self::deposit_event(RawEvent::ErrorRefNotFound(h));
                        return Err("Reference Hash not found");
                    },
                }
                <Status<T>>::remove(&h);
                <Status<T>>::insert(&h, s);
                Self::deposit_event(RawEvent::OrderCompleted(h));
                Ok(())
            }
            /// Used by the buyer to accept or reject (TODO) the invoice that was raised by the seller.
            fn accept_prefunded_invoice(o: T::AccountId, h: T::Hash, s: OrderStatus) -> Result {
                // check that this is the fulfiller
                match Self::order(&h) {
                    Some(order) => {
                        // check the currenct status
                        match Self::status(&h).unwrap() {
                            5 => {
                                match s {
                                    3 => {
                                        // Invoice is disputed. TODO provide the ability to change the invoice and resubmit
                                        Self::deposit_event(RawEvent::ErrorNotImplmented());
                                        return Err("TODO!");
                                    },
                                    6 => {
                                        // Invoice Accepted. Now pay-up!.
                                        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::settle_prefunded_invoice(o.clone(), h)?;
                                        Self::deposit_event(RawEvent::InvoiceSettled(h));
                                    },
                                    _ => {
                                        // All other states are not allowed
                                        Self::deposit_event(RawEvent::ErrorStatusNotAllowed(h, s));
                                        return Err("The order has an unkown state!");
                                    },
                                }
                                // Update the status in this module
                            },
                            _ => {
                                Self::deposit_event(RawEvent::ErrorOrderStatus(h));
                                return Err("The order has an unkown state!");
                            },
                        }
                    },
                    None => {
                        Self::deposit_event(RawEvent::ErrorRefNotFound(h));
                        return Err("Reference Hash not found");
                    },
                }
                
                <Status<T>>::remove(&h);
                <Status<T>>::insert(&h, s);
                
                Ok(())
            }
            /// This is used by any party that wants to accept a market order in whole or part. 
            /// This is non-blocking and can accept many applicants
            fn postulate_simple_prefunded_open_order() -> Result {
                Ok(())
            }
        }
        
        decl_event!(
            pub enum Event<T>
            where
            AccountId = <T as system::Trait>::AccountId,
            BlockNumber = <T as system::Trait>::BlockNumber,
            Hash = <T as system::Trait>::Hash,
            AccountBalance = AccountBalanceOf<T>
            {
                // Positive Messages
                // Test(Vec<ItemDetailsStruct>),
                OrderCreated(AccountId, AccountId, Hash),
                OrderCreatedForApproval(AccountId, AccountId, Hash),
                OrderStatusUpdate(Hash, ApprovalStatus),
                OrderCompleted(Hash),
                InvoiceSettled(Hash),
                // Error Messages      
                ErrorNotApprover(AccountId, Hash),
                ErrorHashExists(Hash),
                ErrorCannotBeBoth(AccountId),
                ErrorNotFulfiller(AccountId, Hash),
                ErrorNotCommander(AccountId, Hash),
                ErrorURNobody(AccountId),
                ErrorOrderStatus(Hash),
                ErrorApprStatus(Hash),
                ErrorApproved(Hash),
                ErrorRefNotFound(Hash),
                ErrorStatusNotAllowed(Hash, OrderStatus),
                ErrorFulfiller(Hash),
                ErrorAmount(AccountBalance),
                ErrorShortDeadline(BlockNumber, BlockNumber),
                ErrorShortDueDate(BlockNumber, BlockNumber),
                ErrorNotImplmented(),
                
                // External Positive Messages - Prefunding & Accounting
                // PrefundingDeposit(AccountId, i128, BlockNumber),
                // PrefundingCancelled(AccountId, Hash),
                // PrefundingLockSet(AccountId, Hash),
                // PrefundingCompleted(AccountId),                
                // InvoiceIssued(Hash),
                // LegderUpdate(AccountId, u64, i128, u128),
                
                // Error Messages - Prefunding & Accounting
                ErrorLockNotAllowed(Hash),
                ErrorOverflow(u64),
                ErrorGlobalOverflow(),
                ErrorInsufficientFunds(AccountId, u128, u128, u128),
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