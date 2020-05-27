use support::dispatch::Result;
// use parity_codec::{ Encode, Decode, Codec, HasCompact};
use runtime_primitives::traits::{ Member};
use rstd::prelude::Vec;
// use rstd::ops::*;

pub trait Posting<AccountId,Hash,BlockNumber> {

    // type Account: Member + PartialOrd + Copy;
    type Account: Member + Copy;
    // type AccountBalance: Member + PartialOrd + Copy + Into<i128> + SimpleArithmetic;
    type AccountBalance: Member + Copy + Into<i128>;

    fn handle_multiposting_amounts(
        o: AccountId,
        fwd: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        rev: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        trk: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>) -> Result;

}