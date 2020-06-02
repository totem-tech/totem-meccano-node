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

    fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash;

}

pub trait Encumbrance<AccountId,Hash,BlockNumber> {
    
    // Lock type
    type UnLocked: Member + Copy;

    fn prefunding_for(who: AccountId, recipient: AccountId, amount: u128, deadline: BlockNumber) -> Result;
    fn send_simple_invoice(o: AccountId, p: AccountId, n: i128, h: Hash) -> Result;
    fn settle_prefunded_invoice(o: AccountId, h: Hash) -> Result;
    fn check_ref_owner(o: AccountId, h: Hash) -> bool;
    fn set_release_state(o: AccountId, o_lock: Self::UnLocked, h: Hash, sender: bool) -> Result;
    fn check_ref_beneficiary(o: AccountId, h: Hash) -> bool;
    fn unlock_funds_for_owner(o: AccountId, h: Hash) -> Result;

}