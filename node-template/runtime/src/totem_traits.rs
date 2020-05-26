use support::dispatch::Result;
// use parity_codec::{ Encode, Decode, Codec };
use runtime_primitives::traits::{Zero, One};
use rstd::prelude::Vec;
use rstd::ops::*;

// pub trait Posting<AccountId,Hash,BlockNumber>: Clone + Decode + Encode + Codec + Eq {
pub trait Posting<AccountId,Hash,BlockNumber> {

    type Account: Clone + PartialOrd;
    type AccountBalance: Clone + PartialOrd + Add + Sub + Mul + Div + Neg + Zero + One;

    fn handle_multiposting_amounts(
        o: AccountId,
        fwd: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        rev: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>, 
        trk: Vec<(AccountId, Self::Account, Self::AccountBalance, bool, Hash, BlockNumber, BlockNumber)>) -> Result;

}