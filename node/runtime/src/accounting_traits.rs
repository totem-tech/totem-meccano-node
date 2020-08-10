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

use parity_codec::{ Encode, Decode };
// use codec::{ Encode, Decode }; // v2
use support::dispatch::Result;
// use frame_support::dispatch::DispatchResult; //v2
use runtime_primitives::traits::{ Member };
// use sp_runtime::traits::{ Member }; // v2
use rstd::prelude::Vec;
// use sp_std::prelude::Vec; //v2

pub trait Posting<AccountId,Hash,BlockNumber> {

    type Account: Member + Copy + Eq;
    type PostingIndex: Member + Copy + Into<u128> + Encode + Decode + Eq;
    type LedgerBalance: Member + Copy + Into<i128> + Encode + Decode + Eq;

    fn handle_multiposting_amounts(
        o: AccountId,
        fwd: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        rev: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        trk: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>) -> Result;

    fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash;

}