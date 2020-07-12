use support::{ dispatch::Result };

pub trait Storing<Hash> {
    fn claim_data(r: Hash, d: Hash) -> Result;
}