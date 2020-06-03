use parity_codec::{ Encode, Decode };
use support::dispatch::Result;
use runtime_primitives::traits::{ Member};
use rstd::prelude::Vec;

pub trait Posting<AccountId,Hash,BlockNumber> {

    type Account: Member + Copy;
    type AccountBalance: Member + Copy + Into<i128> + Encode + Decode;

    fn handle_multiposting_amounts(
        o: AccountId,
        fwd: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        rev: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        trk: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>) -> Result;

    fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash;

}