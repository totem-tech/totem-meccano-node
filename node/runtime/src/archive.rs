// Copyright 2019 Chris D'Costa
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

use support::{decl_event, decl_module, dispatch::Result};
use system::ensure_signed;
use rstd::prelude::*;
use node_primitives::Hash;

// Totem crates
use crate::timekeeping;

pub trait Trait: timekeeping::Trait + system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type RecordType = u16;

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        
        /// Archive types
        /// 1000
        /// 2000
        /// 3000 Activities (previously Projects)
        /// 4000 Timekeeping
        /// 5000
        /// 6000
        /// 7000
        /// 8000
        /// 9000
        fn archive_record(
            origin,
            record_type: RecordType, 
            record_hash: Hash, 
            archive: bool
        ) -> Result {
            // check signed
            let who = ensure_signed(origin)?;
            
            // check which type of record
            match record_type {
                4000 => {
                    // module specific archive handling
                    <timekeeping::Module<T>>::validate_and_archive(who.clone(), record_hash, archive)?;

                    // issue event
                    Self::deposit_event(RawEvent::RecordArchived(4000, who, record_hash, archive));
                },
                _ => return Err("Unknown or unimplemented record type. Cannot archive record"),
            }

            Ok(())
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        Archival = bool,
    {
        RecordArchived(RecordType, AccountId, Hash, Archival),
    }
);