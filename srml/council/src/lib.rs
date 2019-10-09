// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! # Council Module
//!
//! The Council module provides tools to manage the council and proposals. The main components are:
//!
//! - **Council Seats:** Election of councillors.
//! 	- [`seats::Trait`](./seats/trait.Trait.html)
//! 	- [`Call`](./seats/enum.Call.html)
//! 	- [`Module`](./seats/struct.Module.html)
//! - **Council Motions:** Voting as a body to dispatch calls from the `Council` origin.
//! 	- [`motions::Trait`](./motions/trait.Trait.html)
//! 	- [`Call`](./motions/enum.Call.html)
//! 	- [`Module`](./motions/struct.Module.html)
//! - **Council Voting:** Proposals sent to the [Democracy module](../srml_democracy/index.html) for referenda.
//! 	- [`voting::Trait`](./voting/trait.Trait.html)
//! 	- [`Call`](./voting/enum.Call.html)
//! 	- [`Module`](./voting/struct.Module.html)
//!
//! ## Overview
//!
//! The Council module provides functionality to handle:
//!
//! - Voting in and maintenance of council members.
//! - Proposing, vetoing, and passing of motions.
//!
//! The council is an on-chain entity comprised of a set of account IDs, with the role of representing
//! passive stakeholders. Its primary tasks are to propose sensible referenda and thwart any uncontroversially
//! dangerous or malicious referenda.
//!
//! ### Terminology
//!
//! #### Council Motions (motions.rs)
//!
//! _Motions_ handle internal proposals that are only proposed and voted upon by _councillors_.
//! Each proposal has a minimum threshold of yay votes that it needs to gain to be enacted.
//!
//! - **Council motion:** A mechanism used to enact a proposal.
//! - **Proposal:** A submission by a councillor. An initial vote of yay from that councillor is applied.
//! - **Vote:** A vote of yay or nay from a councillor on a single proposal. Councillors may change their vote but a
//!   duplicate vote will return an error.
//!
//! Upon each vote, if the threshold is reached, the proposal is dispatched from the `Council` origin. Similarly,
//! if the number of nay votes is high enough such that it could not pass even if all other councillors
//! (including those who have not voted) voted yay, the proposal is dropped.
//!
//! Note that a council motion has a special origin type, [`seats::Origin`](./motions/enum.Origin.html), that limits
//! which calls can be effectively dispatched.
//!
//! #### Council Voting (voting.rs)
//!
//! _Voting_ handles councillor proposing and voting. Unlike motions, if a proposal is approved,
//! it is elevated to the [Democracy module](../srml_democracy/index.html) as a referendum.
//!
//! - **Proposal validity:** A council proposal is valid when it's unique, hasn't yet been vetoed, and
//! when the proposing councillor's term doesn't expire before the block number when the proposal's voting period ends.
//! A proposal is a generic type that can be _dispatched_ (similar to variants of the `Call` enum in each module).
//! - **Proposal postponement:** Councillors may postpone a council proposal from being approved or rejected.
//! Postponement is equivalent to a veto, which only lasts for the cooloff period.
//! - **Cooloff period:** Period, in blocks, for which a veto is in effect.
//! - **Referendum:** The means of public voting on a proposal.
//! - **Veto:** A council member may veto any council proposal that exists. A vetoed proposal that's valid is set
//! aside for a cooloff period. The vetoer cannot re-veto or propose the proposal again until the veto expires.
//! - **Elevation:** A referendum can be elevated from the council to a referendum. This means it has
//! been passed to the Democracy module for a public vote.
//! - **Referendum cancellation:** At the end of a given block we cancel all elevated referenda whose voting period
//! ends at that block and where the outcome of the vote tally was a unanimous vote to cancel the referendum.
//! - **Voting process to elevate a proposal:** At the end of a given block we tally votes for expiring referenda.
//! Referenda that are passed (yay votes are greater than nay votes plus abstainers) are sent to the Democracy
//! module for a public referendum. If there are no nay votes (abstention is acceptable), then the proposal is
//! tabled immediately. Otherwise, there will be a delay period. If the vote is unanimous, then the public
//! referendum will require a vote threshold of supermajority against to prevent it. Otherwise,
//! it is a simple majority vote. See [`VoteThreshold`](../srml_democracy/enum.VoteThreshold.html) in the
//! Democracy module for more details on how votes are approved.
//!
//! As opposed to motions, proposals executed through the Democracy module have the
//! root origin, which gives them the highest privilege.
//!
//! #### Council Seats (seats.rs)
//!
//! _Seats_ handles the selection of councillors. The selection of council seats is a circulating procedure,
//! regularly performing approval voting to accept a new council member and remove those whose period has ended.
//! Each tally (round of voting) is divided into two time periods:
//!
//!   - **Voting period:** In which any stakeholder can vote for any of the council candidates.
//!   - **Presentation period:** In which voting is no longer allowed, and stakeholders can _present_ a candidate
//!   and claim that a particular candidate has won a seat.
//!
//! A tally is scheduled to execute based on the number of desired and free seats in the council.
//!
//! Both operations have associated bonds and fees that need to be paid based on the complexity of the transaction.
//! See [`set_approvals`](./seats/enum.Call.html#variant.set_approvals) and
//! [`submit_candidacy`](./seats/enum.Call.html#variant.submit_candidacy) for more information.
//!
//! Upon the end of the presentation period, the leaderboard is finalized and sorted based on the approval
//! weight of the _presented_ candidates.
//! Based on the configurations of the module, `N` top candidates in the leaderboard are immediately recognized
//! as councillors (where `N` is `desired_seats - active_council.len()`) and runners-up are kept in
//! the leaderboard as carry for the next tally to compete again. Similarly, councillors whose term has ended
//! are also removed.
//!
//! - **Vote index**: A counter indicating the number of tally rounds already applied.
//! - **Desired seats:** The number of seats on the council.
//! - **Candidacy bond:** Bond required to be a candidate. The bond is returned to all candidates at the end
//! of the tally if they have won the tally (i.e. have become a councillor).
//! - **Leaderboard:** A list of candidates and their respective votes in an election, ordered low to high.
//! - **Presentation:** The act of proposing a candidate for insertion into the leaderboard. Presenting is
//! `O(|number_of_voters|)`, so the presenter must be slashable and will be slashed for duplicate or invalid
//! presentations. Presentation is only allowed during the "presentation period," after voting has closed.
//! - **Voting bond:** Bond required to be permitted to vote. Must be held because many voting operations affect
//! storage. The bond is held to discourage abuse.
//! - **Voting:** Process of inserting approval votes into storage. Can be called by anyone, given they submit
//! an appropriate list of approvals. A bond is reserved from a voter until they retract or get reported.
//! - **Inactive voter**: A voter whose approvals are now invalid. Such voters can be _reaped_ by other voters
//!   after an `inactivity_grace_period` has passed from their last known activity.
//! - **Reaping process:** Voters may propose the removal of inactive voters, as explained above. If the claim is not
//! valid, the _reaper_ will be slashed and removed as a voter. Otherwise, the _reported_ voter is removed. A voter
//! always gets his or her bond back upon being removed (either through _reaping_ or _retracting_).
//!
//! ### Goals
//!
//! The Council module in Substrate is designed to make the following possible:
//!
//! - Create council proposals by councillors using the council motion mechanism.
//! - Validate council proposals.
//! - Tally votes of council proposals by councillors during the proposal's voting period.
//! - Veto (postpone) council proposals for a cooloff period through abstention by councillors.
//! - Elevate council proposals to start a referendum.
//! - Execute referenda once their vote tally reaches the vote threshold level of approval.
//! - Manage candidacy, including voting, term expiration, and punishment.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! The dispatchable functions in the Council module provide the functionality that councillors need.
//! See the `Call` enums from the Motions, Seats, and Voting modules for details on dispatchable functions.
//!
//! ### Public Functions
//!
//! The public functions provide the functionality for other modules to interact with the Council module.
//! See the `Module` structs from the Motions, Seats, and Voting modules for details on public functions.
//!
//! ## Usage
//!
//! ### Council Seats: Councillor Election Procedure
//!
//! A Council seat vote can proceed as follows:
//!
//! 1. Candidates submit themselves for candidacy.
//! 2. Voting occurs.
//! 3. Voting period ends and presentation period begins.
//! 4. Candidates are presented for the leaderboard.
//! 5. Presentation period ends, votes are tallied, and new council takes effect.
//! 6. Candidate list is cleared (except for a defined number of carries) and vote index increased.
//!
//! ### Council Votes: Proposal Elevation Procedure
//!
//! A council motion elevation would proceed as follows:
//!
//! 1. A councillor makes a proposal.
//! 2. Other councillors vote yay or nay or abstain.
//! 3. At the end of the voting period, the votes are tallied.
//! 4. If it has passed, then it will be sent to the Democracy module with the vote threshold parameter.
//!
//! ### Example
//!
//! This code snippet uses the `is_councillor` public function to check if the calling user
//! is an active councillor before proceeding with additional runtime logic.
//!
//! ```
//! use srml_support::{decl_module, ensure, dispatch::Result};
//! use system::ensure_signed;
//! use srml_council::motions;
//!
//! pub trait Trait: motions::Trait + system::Trait {}
//!
//! decl_module! {
//! 	pub struct Module<T: Trait> for enum Call where origin: <T as system::Trait>::Origin {
//!
//! 		pub fn privileged_function(origin) -> Result {
//! 			// Get the sender AccountId
//! 			let sender = ensure_signed(origin)?;
//!
//! 			// Check they are an active councillor
//!				ensure!(<motions::Module<T>>::is_councillor(&sender),
//!					"Must be a councillor to call this function");
//!				
//!				// Do some privileged operation here...
//!
//!				// Return `Ok` at the end
//! 			Ok(())
//! 		}
//! 	}
//! }
//! # fn main() { }
//! ```
//!
//! ## Genesis Config
//!
//! The Council module depends on the `GenesisConfig`.
//!
//! - [Seats](./seats/struct.GenesisConfig.html)
//! - [Voting](./voting/struct.GenesisConfig.html)
//!
//! ## Related Modules
//!
//! - [Democracy](../srml_democracy/index.html)
//! - [Staking](../srml_staking/index.html)
//!
//! ## References
//!
//! - [Polkadot wiki](https://wiki.polkadot.network/en/latest/polkadot/learn/governance/) on governance.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit="128"]

pub mod motions;
pub mod seats;

pub use crate::seats::{Trait, Module, RawEvent, Event, VoteIndex};

/// Trait for type that can handle incremental changes to a set of account IDs.
pub trait OnMembersChanged<AccountId> {
	/// A number of members `new` just joined the set and replaced some `old` ones.
	fn on_members_changed(new: &[AccountId], old: &[AccountId]);
}

impl<T> OnMembersChanged<T> for () {
	fn on_members_changed(_new: &[T], _old: &[T]) {}
}

#[cfg(test)]
mod tests {
	// These re-exports are here for a reason, edit with care
	pub use super::*;
	pub use runtime_io::with_externalities;
	use support::{impl_outer_origin, impl_outer_event, impl_outer_dispatch, parameter_types};
	use support::traits::Get;
	pub use primitives::{H256, Blake2Hasher, u32_trait::{_1, _2, _3, _4}};
	pub use sr_primitives::traits::{BlakeTwo256, IdentityLookup};
	pub use sr_primitives::testing::{Digest, DigestItem, Header};
	pub use sr_primitives::Perbill;
	pub use {seats, motions};
	use std::cell::RefCell;

	impl_outer_origin! {
		pub enum Origin for Test {
			motions<T>
		}
	}

	impl_outer_event! {
		pub enum Event for Test {
			balances<T>, democracy<T>, seats<T>, motions<T>,
		}
	}

	impl_outer_dispatch! {
		pub enum Call for Test where origin: Origin {
			type Error = Error;

			balances::Balances,
			democracy::Democracy,
		}
	}

	// Workaround for https://github.com/rust-lang/rust/issues/26925. Remove when sorted.
	#[derive(Clone, Eq, PartialEq, Debug)]
	pub struct Test;
	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: u32 = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Call = ();
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type WeightMultiplierUpdate = ();
		type Event = Event;
		type Error = Error;
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type MaximumBlockLength = MaximumBlockLength;
		type AvailableBlockRatio = AvailableBlockRatio;
		type Version = ();
	}
	parameter_types! {
		pub const ExistentialDeposit: u64 = 0;
		pub const TransferFee: u64 = 0;
		pub const CreationFee: u64 = 0;
		pub const TransactionBaseFee: u64 = 1;
		pub const TransactionByteFee: u64 = 0;
	}
	impl balances::Trait for Test {
		type Balance = u64;
		type OnNewAccount = ();
		type OnFreeBalanceZero = ();
		type Event = Event;
		type TransactionPayment = ();
		type TransferPayment = ();
		type DustRemoval = ();
		type Error = Error;
		type ExistentialDeposit = ExistentialDeposit;
		type TransferFee = TransferFee;
		type CreationFee = CreationFee;
		type TransactionBaseFee = TransactionBaseFee;
		type TransactionByteFee = TransactionByteFee;
		type WeightToFee = ();
	}
	parameter_types! {
		pub const LaunchPeriod: u64 = 1;
		pub const VotingPeriod: u64 = 3;
		pub const MinimumDeposit: u64 = 1;
		pub const EnactmentPeriod: u64 = 0;
		pub const CooloffPeriod: u64 = 2;
	}
	impl democracy::Trait for Test {
		type Proposal = Call;
		type Event = Event;
		type Currency = balances::Module<Self>;
		type EnactmentPeriod = EnactmentPeriod;
		type LaunchPeriod = LaunchPeriod;
		type EmergencyVotingPeriod = VotingPeriod;
		type VotingPeriod = VotingPeriod;
		type MinimumDeposit = MinimumDeposit;
		type ExternalOrigin = motions::EnsureProportionAtLeast<_1, _2, u64>;
		type ExternalMajorityOrigin = motions::EnsureProportionAtLeast<_2, _3, u64>;
		type EmergencyOrigin = motions::EnsureProportionAtLeast<_1, _1, u64>;
		type CancellationOrigin = motions::EnsureProportionAtLeast<_2, _3, u64>;
		type VetoOrigin = motions::EnsureMember<u64>;
		type CooloffPeriod = CooloffPeriod;
	}
	parameter_types! {
		pub const CandidacyBond: u64 = 3;
		pub const CarryCount: u32 = 2;
		pub const InactiveGracePeriod: u32 = 1;
		pub const CouncilVotingPeriod: u64 = 4;
	}
	impl seats::Trait for Test {
		type Event = Event;
		type BadPresentation = ();
		type BadReaper = ();
		type BadVoterIndex = ();
		type LoserCandidate = ();
		type OnMembersChanged = CouncilMotions;
		type CandidacyBond = CandidacyBond;
		type VotingBond = VotingBond;
		type VotingFee = VotingFee;
		type PresentSlashPerVoter = PresentSlashPerVoter;
		type CarryCount = CarryCount;
		type InactiveGracePeriod = InactiveGracePeriod;
		type CouncilVotingPeriod = CouncilVotingPeriod;
		type DecayRatio = DecayRatio;
	}
	impl motions::Trait for Test {
		type Origin = Origin;
		type Proposal = Call;
		type Event = Event;
	}

	pub struct ExtBuilder {
		balance_factor: u64,
		decay_ratio: u32,
		voting_fee: u64,
		voter_bond: u64,
		bad_presentation_punishment: u64,
		with_council: bool,
	}

	impl Default for ExtBuilder {
		fn default() -> Self {
			Self {
				balance_factor: 1,
				decay_ratio: 24,
				voting_fee: 0,
				voter_bond: 0,
				bad_presentation_punishment: 1,
				with_council: false,
			}
		}
	}

	impl ExtBuilder {
		pub fn with_council(mut self, council: bool) -> Self {
			self.with_council = council;
			self
		}
		pub fn balance_factor(mut self, factor: u64) -> Self {
			self.balance_factor = factor;
			self
		}
		pub fn decay_ratio(mut self, ratio: u32) -> Self {
			self.decay_ratio = ratio;
			self
		}
		pub fn voting_fee(mut self, fee: u64) -> Self {
			self.voting_fee = fee;
			self
		}
		pub fn bad_presentation_punishment(mut self, fee: u64) -> Self {
			self.bad_presentation_punishment = fee;
			self
		}
		pub fn voter_bond(mut self, fee: u64) -> Self {
			self.voter_bond = fee;
			self
		}
		pub fn set_associated_consts(&self) {
			VOTER_BOND.with(|v| *v.borrow_mut() = self.voter_bond);
			VOTING_FEE.with(|v| *v.borrow_mut() = self.voting_fee);
			PRESENT_SLASH_PER_VOTER.with(|v| *v.borrow_mut() = self.bad_presentation_punishment);
			DECAY_RATIO.with(|v| *v.borrow_mut() = self.decay_ratio);
		}
		pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
			self.set_associated_consts();
			let mut t = system::GenesisConfig::default().build_storage::<Test>().unwrap();
			balances::GenesisConfig::<Test>{
				balances: vec![
					(1, 10 * self.balance_factor),
					(2, 20 * self.balance_factor),
					(3, 30 * self.balance_factor),
					(4, 40 * self.balance_factor),
					(5, 50 * self.balance_factor),
					(6, 60 * self.balance_factor)
				],
				vesting: vec![],
			}.assimilate_storage(&mut t).unwrap();
			seats::GenesisConfig::<Test> {
				active_council: if self.with_council { vec![
					(1, 10),
					(2, 10),
					(3, 10)
				] } else { vec![] },
				desired_seats: 2,
				presentation_duration: 2,
				term_duration: 5,
			}.assimilate_storage(&mut t).unwrap();
			runtime_io::TestExternalities::new(t)
		}
	}

	pub type System = system::Module<Test>;
	pub type Balances = balances::Module<Test>;
	pub type Democracy = democracy::Module<Test>;
	pub type Council = seats::Module<Test>;
	pub type CouncilMotions = motions::Module<Test>;
}
