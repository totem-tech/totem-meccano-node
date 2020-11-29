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

/// The purpose of this module is to provide a decentralised authority for data storage
/// In Totem we require an off-chain searchable database that may end up containing billions of records. 
/// IPFS is not a solution as the type of data to be stored may be queried, editied, and each time IPFS cannot overwrite or update existing datasets.
/// Additionally IPFS may drop files that are not considered current, used or needed, which is not ideal for static records like invoices.

/// We wanted a solution where permission for storing an editing data should not be dependent on third-party authentication and access
/// was global, recoverable and self-sovereign.

/// Bonsai is a simple protocol, for allowing independent databases to come to a consensus on content. 
/// It works by assuming that the data to be stored must be previously authenticated by it's owner on-chain

/// This is done in the following way:
/// Firstly, a reference to the record is created either on-chain or offchain by an account which immediately becomes it's owner.
/// The reference is a hash (H256) with sufficient entropy to be unique per the record.
/// A transaction is sent to the blockchain at some point associating the reference to an address for the first time.
/// The reference is considered to be the key to some other data which is not suitable for onchain storage, but will be stored in an offchain database.
/// The offchain database will only accept new or changing records, provided that it can 
/// a) find the reference hash onchain, and 
/// b) an associated data-hash which it also finds on chain with a hash of the incoming data.
/// The data may be plaintext or encrypted, neither matters as long as the hash of this data matches onchain data-hash.
/// As the on-chain transaction validates the signature, the off-chain database does not need to authenticate the client that communicates 
/// the insertion or change request as it has already been "pre-authorised" by the blockchain runtime.
/// Totem believes there is a fee market for storage in this model.

/// Process
/// A third party database receives a request to store some data. The Database queries the blockchain to find out:
/// 1. does the reference hash exist on chain and of it does, then collect the associated data-hash also stored onchain
/// Upon confirmation the reference hash exists, hashing the received data and compare the data-hash to the one found on chain. If it does not match, then do nothing 
/// (effectively rejecting the attempt to store the data), and if it does match then store the data using the reference hash as the key
/// 3. in the event that an reference hash already exists, the data-hash obtained from the blockchain is always king. Provided it matches, overwrite exiting data.

use parity_codec::{Encode};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap};
use substrate_primitives::H256;
use system::{self, ensure_signed};
use rstd::prelude::*;
use runtime_primitives::traits::{Hash, Convert};

// Totem crates
use crate::bonsai_traits::{ Storing };
use crate::orders_traits::{Validating as OrderValidating};
use crate::timekeeping_traits::{Validating as TimeValidating};
use crate::projects_traits::{Validating as ProjectValidating};

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    // type Orders: OrderValidating<Self::AccountId,Self::Hash>;
    type Timekeeping: TimeValidating<Self::AccountId,Self::Hash>;
    type Projects: ProjectValidating<Self::AccountId,Self::Hash>;
    type Orders: OrderValidating<Self::AccountId,Self::Hash>;
    type BonsaiConversions: 
    Convert<Self::Hash, H256> +
    Convert<Self::BlockNumber, u64> +
    Convert<u64, Self::BlockNumber> +
    Convert<H256, Self::Hash>;
}

pub type RecordType = u16;

decl_storage! {
    trait Store for Module<T: Trait> as BonsaiModule {
        // Bonsai Storage
        IsValidRecord get(is_valid_record): map T::Hash => Option<T::Hash>; 
        // Hacky workaround for inability of RPC to query transaction by hash
        IsStarted get(is_started): map T::Hash => Option<T::BlockNumber>; // maps to current block number allows interrogation of errors
        IsSuccessful get(is_successful): map T::Hash => Option<T::BlockNumber>; // future block number beyond which the Hash should deleted
        TxList get(tx_list):  map T::Hash => Vec<T::Hash>; // Tracking to ensure that we can perform housekeeping on finalization of block 
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        
        /// This function stores a record hash for BONSAI 2FA for couchDB
        ///
        /// Record types are the same as the Archive Record Types
        /// * 3000 Activities (previously Projects)
        /// * 4000 Timekeeping
        /// * 5000 Orders
        /// 
        fn update_record(
            origin,
            record_type: RecordType, 
            key: T::Hash,
            bonsai_token: T::Hash 
        ) -> Result {
            // check transaction signed
            let who = ensure_signed(origin)?;
            
            match Self::check_remote_ownership(who.clone(), key.clone(), bonsai_token.clone(), record_type.clone()) {
                Ok(_) => {
                    Self::insert_record(key.clone(), bonsai_token.clone())?;
                },
                Err(e) => {
                    return Err(e);
                },
            }
            Ok(())
        }
        
        fn on_finalize_example(origin) -> Result {
            let _who = ensure_signed(origin)?;
            let current_block: T::BlockNumber = <system::Module<T>>::block_number();
            let current: u64 = <T::BonsaiConversions as Convert<T::BlockNumber, u64>>::convert(current_block);
            // Get all hashes
            let default_bytes = b"nobody can save fiat currency now";
            let list_key: T::Hash = T::Hashing::hash(default_bytes.encode().as_slice());
            
            if <TxList<T>>::exists(&list_key) {
                let hashes: Vec<T::Hash> = Self::tx_list(&list_key);
                // check which storage the hashes come from and hashes that are old
                for i in hashes {
                    
                    let key: T::Hash = i.clone();
                    
                    match Self::is_started(&key) {
                        Some(block) => {
                            
                            let mut target_block: u64 = <T::BonsaiConversions as Convert<T::BlockNumber, u64>>::convert(block);
                            target_block = target_block + 172800u64; 
                            
                            // let mut target_deletion_block: T::BlockNumber = <T::BonsaiConversions as Convert<u64, T::BlockNumber>>::convert(target_block);
                            // cleanup 30 Days from when the transaction started, but did not complete
                            
                            // It's possible this comparison is not working
                            if current >= target_block {
                                <IsStarted<T>>::remove(key.clone());
                            } else {
                                ();
                            }
                        },
                        None => {
                            match Self::is_successful(&key) {
                                Some(block) => {
                                    let target_block: u64 = <T::BonsaiConversions as Convert<T::BlockNumber, u64>>::convert(block);
                                    if current >= target_block {
                                        <IsSuccessful<T>>::remove(key.clone());
                                    } else {
                                        ();
                                    }       
                                },
                                None => (),
                            }
                        },
                    }
                    <TxList<T>>::mutate(&list_key, |tx_list| tx_list.retain(|v| {v != &key}));
                }
            } else {
                ();
            }
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn check_remote_ownership(o: T::AccountId, k: T::Hash, t: T::Hash, e: RecordType) -> Result {
        // check which type of record
        // then check that the supplied hash is owned by the signer of the transaction
        match e {
            3000 => {
                if let false = <<T as Trait>::Projects as ProjectValidating<T::AccountId, T::Hash>>::is_project_owner(o.clone(), k.clone()) {
                    Self::deposit_event(RawEvent::ErrorRecordOwner(t));
                    return Err("You cannot add a record you do not own");
                }
            },
            4000 => {
                if let false = <<T as Trait>::Timekeeping as TimeValidating<T::AccountId, T::Hash>>::is_time_record_owner(o.clone(), k.clone()) {
                    Self::deposit_event(RawEvent::ErrorRecordOwner(t));
                    return Err("You cannot add a record you do not own");
                }
            },
            5000 => {
                if let false = <<T as Trait>::Orders as OrderValidating<T::AccountId, T::Hash>>::is_order_party(o.clone(), k.clone()) {
                    Self::deposit_event(RawEvent::ErrorRecordOwner(t));
                    return Err("You cannot add a record you do not own");
                }
            } 
            _ => {
                Self::deposit_event(RawEvent::ErrorUnknownType(t));
                return Err("Unknown or unimplemented record type. Cannot store record");
            },
        }
        
        Ok(())
    }
    
    fn insert_record(k: T::Hash, t: T::Hash) -> Result {
        // TODO implement fee payment mechanism (currently just transaction fee)
        if <IsValidRecord<T>>::exists(&k) {
            // remove store the token. This overwrites any existing hash.
            <IsValidRecord<T>>::remove(k.clone());
        } else {
            ();
        }
        
        <IsValidRecord<T>>::insert(k, t);
        
        Ok(())
    }
    
    fn insert_uuid(u: T::Hash) -> Result {
        
        if <IsSuccessful<T>>::exists(&u) {
            // Throw an error because the transaction already completed
            return Err("Queued transaction already completed");
            
        } else if <IsStarted<T>>::exists(&u) {
            // What happens on error or second use


            // The transaction is now completed successfully update the state change
            // remove from started, and place in successful
            let current_block = <system::Module<T>>::block_number();
            let mut block: u64 = <T::BonsaiConversions as Convert<T::BlockNumber, u64>>::convert(current_block);
            block = block + 172800u64; // cleanup in 30 Days
            let deletion_block: T::BlockNumber = <T::BonsaiConversions as Convert<u64, T::BlockNumber>>::convert(block);
            <IsStarted<T>>::remove(&u);
            <IsSuccessful<T>>::insert(u, deletion_block);
            
        } else {
            // this is a new UUID just starting the transaction
            let current_block = <system::Module<T>>::block_number();
            let default_bytes = b"nobody can save fiat currency now";
            let list_key: T::Hash = T::Hashing::hash(default_bytes.encode().as_slice());
            <TxList<T>>::mutate(list_key, |tx_list| tx_list.push(u));
            <IsStarted<T>>::insert(u, current_block);
            
        }
        Ok(())
    }
}

impl<T: Trait> Storing<T::Hash> for Module<T> {
    fn claim_data(r: T::Hash, d: T::Hash) -> Result {
        Self::insert_record(r.clone(), d.clone())?;
        Ok(())
    }
    fn store_uuid(u: T::Hash) -> Result {
        Self::insert_uuid(u.clone())?;
        Ok(())
    }
}

decl_event!(
    pub enum Event<T>
    where
    Hash = <T as system::Trait>::Hash,
    {
        ErrorRecordOwner(Hash),
        ErrorUnknownType(Hash),
    }
);