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
// use system::ensure_signed;
use system::{self, ensure_signed};
use rstd::prelude::*;

// Totem crates
use crate::timekeeping;
use crate::projects;

// pub trait Trait: system::Trait {()};

pub trait Trait: timekeeping::Trait + projects::Trait + system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type RecordType = u16;
pub type RecordHash = Hash;
pub type DataHash = Hash;

decl_storage! {
    trait Store for Module<T: Trait> as BonsaiModule {
        IsValidRecord get(is_valid_record): map RecordHash => Option<DataHash>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        
        // /
        // / This function stores a record hash for BONSAI 2FA for couchDB
        // /
        // / Record types are the same as the Archive Record Types
        // / * 3000 Activities (previously Projects)
        // / * 4000 Timekeeping
        
        fn update_record(
            origin,
            record_type: RecordType, 
            key: RecordHash,
            token: DataHash 
        ) -> Result {
            // check transaction signed
            let who = ensure_signed(origin)?;
            // let token.clone(): DataHash = token.clone();
            
            // check which type of record
            // then check that the supplied hash is owned by the signer of the transaction
            match record_type {
                3000 => {
                    match <projects::Module<T>>::check_project_owner(who.clone(), key.clone()) {
                        true => (), // Do nothing
                        false => return Err("You cannot add a record you do not own"),
                    }
                },
                4000 => {

                    // Convert from H256 to [u8; 32]. Might need dereferencing in other contexts
                    // let key_copy2: T::Hash  = key.clone();

                    match <timekeeping::Module<T>>::check_time_record_owner(who.clone(), key.clone()) {
                        true => (), // Do nothing
                        false => return Err("You cannot add a record you do not own"),
                    }
                },
                _ => return Err("Unknown or unimplemented record type. Cannot store record"),
            };
            
            // TODO implement fee payment mechanism
            // take the payment for the transaction
            // send the payment to the storage treasury
                                        
            // remove store the token. This overwrites any existing hash.
            <IsValidRecord<T>>::remove(key.clone());
            <IsValidRecord<T>>::insert(key.clone(), token.clone());

            // issue event
            Self::deposit_event(RawEvent::IsValidRecord(record_type, key.clone(), token.clone()));
            
            Ok(())
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    {
        Dummy(AccountId),
        IsValidRecord(RecordType, Hash, Hash),
    }
);