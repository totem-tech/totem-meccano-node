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

// use parity_codec::{Decode, Encode};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageMap};
use node_primitives::Hash;
use substrate_primitives::H256;
use system::{self, ensure_signed};
use rstd::prelude::*;
use runtime_primitives::traits::{  Convert };

// Totem crates
use crate::timekeeping;
use crate::projects;
use crate::bonsai_traits::{ Storing };

pub trait Trait: timekeeping::Trait + projects::Trait + system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Conversions: 
    Convert<Self::Hash, H256> +
    Convert<H256, Self::Hash>;
}

pub type RecordType = u16;

decl_storage! {
    trait Store for Module<T: Trait> as BonsaiModule {
        IsValidRecord get(is_valid_record): map Hash => Option<Hash>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        
        ///
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
            key: H256,
            token: H256 
        ) -> Result {
            // check transaction signed
            let who = ensure_signed(origin)?;
            
            match Self::check_remote_ownership(who.clone(), key.clone(), token.clone(), record_type.clone()) {
                Ok(_) => {
                    let key_hash: T::Hash = <T::Conversions as Convert<H256, T::Hash>>::convert(key);
                    let token_hash: T::Hash = <T::Conversions as Convert<H256, T::Hash>>::convert(token);
                    Self::insert_record(key_hash.clone(), token_hash.clone())?;
                    
                    // Self::deposit_event(RawEvent::Bonsai(record_type, key_hash, token_hash));
                },
                Err(e) => {
                    return Err(e);
                },
            }
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    
    fn check_remote_ownership(o: T::AccountId, k: H256, t: H256, e: RecordType) -> Result {
        // check which type of record
        // then check that the supplied hash is owned by the signer of the transaction
        match e {
            3000 => {
                match <projects::Module<T>>::check_project_owner(o.clone(), k.clone()) {
                    true => (), // Do nothing
                    false => {
                        let key_hash: T::Hash = <T::Conversions as Convert<H256, T::Hash>>::convert(k);
                        let token_hash: T::Hash = <T::Conversions as Convert<H256, T::Hash>>::convert(t);
                        
                        Self::deposit_event(RawEvent::ErrorRecordOwner(o, key_hash, token_hash));
                        return Err("You cannot add a record you do not own");
                    },
                }
            },
            4000 => {
                match <timekeeping::Module<T>>::check_time_record_owner(o.clone(), k.clone()) {
                    true => (), // Do nothing
                    false => {
                        let key_hash: T::Hash = <T::Conversions as Convert<H256, T::Hash>>::convert(k);
                        let token_hash: T::Hash = <T::Conversions as Convert<H256, T::Hash>>::convert(t);
                        
                        Self::deposit_event(RawEvent::ErrorRecordOwner(o, key_hash, token_hash));
                        return Err("You cannot add a record you do not own");
                    },
                }
            },
            5000 => {
                unimplemented!();
            }
            _ => {
                Self::deposit_event(RawEvent::ErrorUnknownType(e));
                return Err("Unknown or unimplemented record type. Cannot store record");
            },
        };
        
        Ok(())
    }
    
    fn insert_record(k: T::Hash, t: T::Hash) -> Result {
        // TODO implement fee payment mechanism (currently just transaction fee)
        let key_h256: H256 = <T::Conversions as Convert<T::Hash, H256>>::convert(k);
        let token_h256: H256 = <T::Conversions as Convert<T::Hash, H256>>::convert(t);
        
        if <IsValidRecord<T>>::exists(key_h256) {
            // remove store the token. This overwrites any existing hash.
            <IsValidRecord<T>>::remove(key_h256.clone());
        };
        
        <IsValidRecord<T>>::insert(key_h256, token_h256);
        
        Ok(())
    }
}

impl<T: Trait> Storing<T::Hash> for Module<T> {
    fn claim_data(r: T::Hash, d: T::Hash) -> Result {
        Self::insert_record(r, d)?;
        Ok(())
    }
}

decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    Hash = <T as system::Trait>::Hash,
    RecordType = u16,
    {
        Bonsai(RecordType, Hash, Hash),
        ErrorRecordOwner(AccountId, Hash, Hash),
        ErrorUnknownType(RecordType),
    }
);