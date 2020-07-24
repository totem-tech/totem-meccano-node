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

use support::{decl_event, decl_module, dispatch::Result};
use system::ensure_signed;
use rstd::prelude::*;

// Totem crates
use crate::timekeeping_traits::{ Validating as TimeValidating};

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Timekeeping: TimeValidating<Self::AccountId,Self::Hash>;

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
        /// 5000 Orders
        /// 6000
        /// 7000
        /// 8000
        /// 9000
        fn archive_record(
            origin,
            record_type: RecordType, 
            bonsai_token: T::Hash, 
            archive: bool
        ) -> Result {
            // check signed
            let who = ensure_signed(origin)?;
            
            // check which type of record
            match record_type {
                4000 => {
                    // module specific archive handling
                    if let true = <<T as Trait>::Timekeeping as TimeValidating<T::AccountId, T::Hash>>::validate_and_archive(who.clone(), bonsai_token, archive) {
                        // issue event
                        Self::deposit_event(RawEvent::RecordArchived(4000, who, bonsai_token, archive));
                    }
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
        Hash = <T as system::Trait>::Hash,
        Archival = bool,
        RecordType = u16,
    {
        RecordArchived(RecordType, AccountId, Hash, Archival),
    }
);