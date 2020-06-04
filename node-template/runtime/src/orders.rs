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

// buy_or_sell: u16, // 0: buy, 1: sell, extensible
// amount: AccountBalanceOf<T>, // amount should be the sum of all the items untiprices * quantities
// open_closed: bool, // 0: open(true) 1: closed(false)
// order_type: u16, // 0: personal, 1: business, extensible 
// deadline: u64, // prefunding acceptance deadline 
// due_date: u64, // due date is the future delivery date (in blocks) 
type OrderHeader = (u16, i128, bool, u16, u64, u64);

type OrderItem = Vec<(Product, UnitPrice, Quantity)>;


pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Conversions: 
        Convert<i128, AccountBalanceOf<Self>> + 
        Convert<i128, u128> + 
        Convert<AccountBalanceOf<Self>, i128> + 
        Convert<u64, Self::BlockNumber>;
    type Accounting: Posting<Self::AccountId,Self::Hash,Self::BlockNumber>;
    type Prefunding: Encumbrance<Self::AccountId,Self::Hash,Self::BlockNumber>;
}

decl_storage! {
    trait Store for Module<T: Trait> as OrdersModule {
        Owner get(owner): map T::AccountId => Vec<T::Hash>;
        Beneficiary get(beneficiary): map T::AccountId => Vec<T::Hash>;
        Approver get(approver): map T::AccountId => Vec<T::Hash>;
        
        Order get(order): map T::Hash => Option<(T::AccountId,T::AccountId,T::AccountId,u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber)>;
        Details get(details): map T::Hash => OrderItem;
        Status get(status): map T::Hash => Option<OrderStatus>;
        Approved get(approved): map T::Hash => Option<ApprovalStatus>;
        // Order get(order): map T::Hash => Option<(bool,u16)>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
    }
}

impl<T: Trait> Module<T> {
    fn set_approval(a: T::AccountId, h:T::Hash) -> Result {
        // check that the hash exists and that it can be approved by sender
        ensure!(<Status<T>>::exists(&h), "The hash already exists! Try again.");
        Ok(())
    }

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
        deadline: u64, // prefunding acceptance deadline 
        due_date: u64, // due date is the future delivery date (in blocks) 
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
            let prefund_amount: i128 = <T::Conversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount);
            let order_header = (buy_or_sell, prefund_amount, open_closed, order_type, deadline, due_date);
            Self::record_approved_order(commander.clone(), fulfiller.clone(), approver.clone(), order_hash, order_header, order_items)?;

        } else {
            // This is not an error but requires further processing by the approver. Exiting gracefully.
            Self::deposit_event(RawEvent::OrderCreatedForApproval(commander, approver, order_hash));
            ();
        }

        Ok(())
    }
    
    fn record_approved_order(c: T::AccountId, f: T::AccountId, a: T::AccountId, o: T::Hash, h: OrderHeader, i: OrderItem ) -> Result {
        
        // Set Prefunding - do this now, it does not matter if there are errors after this point.
        ensure!(c != f, "Beneficiary must be another account");
        let amount: u128 = <T::Conversions as Convert<i128, u128>>::convert(h.1);
        let balance_amount: AccountBalanceOf<T> = <T::Conversions as Convert<i128, AccountBalanceOf<T>>>::convert(h.1);
        let deadline: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(h.4);
        let due_date: T::BlockNumber = <T::Conversions as Convert<u64, T::BlockNumber>>::convert(h.5);
        // make storage tuple
        let order_hdr: (T::AccountId,T::AccountId,T::AccountId,u16,AccountBalanceOf<T>,bool,u16,T::BlockNumber,T::BlockNumber) = (c.clone(), f.clone(), a.clone(), h.0, balance_amount, h.2, h.3, deadline, due_date); 

        <<T as Trait>::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::prefunding_for(c.clone(), f.clone(), amount, deadline)?;
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
        <Order<T>>::insert(&o, order_hdr);
        <Details<T>>::insert(&o, i);

        // issue events
        Self::deposit_event(RawEvent::OrderCreated(c, f, o));
        
        Ok(())
    }
    
    fn approve_order(a: T::AccountId, h: T::Hash, s: ApprovalStatus) -> Result {
        
        // is the supplied account the approver of the hash supplied?
        match Self::order(&h) {
            Some(order) => {
                if a == order.2 {
                    // check the status being proposed
                    match s {
                        1 | 2 => (), // approved
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
        OrderCreatedForApproval(AccountId, AccountId, Hash),
        OrderStatusUpdate(Hash, ApprovalStatus),
        ErrorNotApprover(AccountId, Hash),
        ErrorApprStatus(Hash),
        ErrorRefNotFound(Hash),
    }
);