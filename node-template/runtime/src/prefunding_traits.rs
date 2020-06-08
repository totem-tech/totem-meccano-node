use support::dispatch::Result;
use runtime_primitives::traits::{ Member};

pub trait Encumbrance<AccountId,Hash,BlockNumber> {
    
    type UnLocked: Member + Copy;

    fn prefunding_for(who: AccountId, recipient: AccountId, amount: u128, deadline: BlockNumber) -> Result;
    fn send_simple_invoice(o: AccountId, p: AccountId, n: i128, h: Hash) -> Result;
    fn settle_prefunded_invoice(o: AccountId, h: Hash) -> Result;
    fn check_ref_owner(o: AccountId, h: Hash) -> bool;
    fn set_release_state(o: AccountId, o_lock: Self::UnLocked, h: Hash, sender: bool) -> Result;
    fn check_ref_beneficiary(o: AccountId, h: Hash) -> bool;
    fn unlock_funds_for_owner(o: AccountId, h: Hash) -> Result;

}