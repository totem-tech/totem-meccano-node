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

//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

use rstd::prelude::*;
use support::construct_runtime;
use substrate_primitives::u32_trait::{_2, _4};
use node_primitives::{
	AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, AuthorityId, Signature, AuthoritySignature
};
use grandpa::fg_primitives::{self, ScheduledChange};
use client::{
	block_builder::api::{self as block_builder_api, InherentData, CheckInherentsResult},
	runtime_api as client_api, impl_runtime_apis
};
use runtime_primitives::{ApplyResult, generic, create_runtime_str};
use runtime_primitives::transaction_validity::TransactionValidity;
use runtime_primitives::traits::{
	BlakeTwo256, Block as BlockT, DigestFor, NumberFor, StaticLookup, AuthorityIdFor, Convert
};
use version::RuntimeVersion;
use council::{motions as council_motions, voting as council_voting};
#[cfg(feature = "std")]
use council::seats as council_seats;
#[cfg(any(feature = "std", test))]
use version::NativeVersion;
use substrate_primitives::OpaqueMetadata;

#[cfg(any(feature = "std", test))]
pub use runtime_primitives::BuildStorage;
pub use consensus::Call as ConsensusCall;
pub use timestamp::Call as TimestampCall;
pub use accounting::Call as AccountingCall;
pub use funding::Call as FundingCall;
pub use balances::Call as BalancesCall;
pub use runtime_primitives::{Permill, Perbill};
pub use support::StorageValue;
pub use staking::StakerStatus;

extern crate sodalite;

// Totem Runtime Modules
mod archive;
mod bonsai;
mod bonsai_traits;
mod boxkeys;
mod orders;
mod orders_traits;
mod prefunding;
mod prefunding_traits;
mod projects;
mod projects_traits;
mod timekeeping;
mod timekeeping_traits;
mod transfer;
// mod crowdsale;
// mod crowdsale_traits;

/// This is the Totem runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	// node runtime name // fork risk, on change
	spec_name: create_runtime_str!("totem-meccano"),
	// team/implementation name
	impl_name: create_runtime_str!("totem-meccano-team"),
	// for block authoring // fork risk, on change
	authoring_version: 1,
	// spec version // fork risk, on change
	spec_version: 15,
    // incremental changes
	impl_version: 4,
	apis: RUNTIME_API_VERSIONS,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

// Totem implemented for converting between Accounting Balances and Internal Balances
pub struct ConversionHandler;

// Basic type conversion
impl ConversionHandler {
	fn signed_to_unsigned(x: i128) -> u128 { x.abs() as u128 }
}

// Takes the AccountBalance and converts for use with BalanceOf<T>
impl Convert<i128, u128> for ConversionHandler {
	fn convert(x: i128) -> u128 { Self::signed_to_unsigned(x) as u128 }
}

// Takes BalanceOf<T> and converts for use with AccountBalance type
impl Convert<u128, i128> for ConversionHandler {
    fn convert(x: u128) -> i128 { x as i128 }
}

// Takes integer u64 and converts for use with AccountOf<T> type or BlockNumber
impl Convert<u64, u64> for ConversionHandler {
    fn convert(x: u64) -> u64 { x }
}

// Takes integer u64 or AccountOf<T> and converts for use with BalanceOf<T> type
impl Convert<u64, u128> for ConversionHandler {
    fn convert(x: u64) -> u128 { x as u128 }
}

// Takes integer i128 and inverts for use mainly with AccountBalanceOf<T> type
impl Convert<i128, i128> for ConversionHandler {
    fn convert(x: i128) -> i128 { x }
}
// Used for extracting a user's balance into an integer for calculations 
impl Convert<u128, u128> for ConversionHandler {
    fn convert(x: u128) -> u128 { x }
}
// Used to convert to associated type UnLocked<T> 
impl Convert<bool, bool> for ConversionHandler {
    fn convert(x: bool) -> bool { x }
}

// Takes Vec<u8> encoded hash and converts for as a LockIdentifier type
impl Convert<Vec<u8>, [u8;8]> for ConversionHandler {
	fn convert(x: Vec<u8>) -> [u8;8] { 
		let mut y: [u8;8] = [0;8];
        for z in 0..8 {
			y[z] = x[z].into();
        };
        return y;
    }
}
// Used to convert hashes 
impl Convert<Hash, Hash> for ConversionHandler {
	fn convert(x: Hash) -> Hash { x }
}

pub struct CurrencyToVoteHandler;

impl CurrencyToVoteHandler {
	fn factor() -> u128 { (Balances::total_issuance() / u64::max_value() as u128).max(1) }
}

impl Convert<u128, u64> for CurrencyToVoteHandler {
	fn convert(x: u128) -> u64 { (x / Self::factor()) as u64 }
}

impl Convert<u128, u128> for CurrencyToVoteHandler {
	fn convert(x: u128) -> u128 { x * Self::factor() }
}

impl system::Trait for Runtime {
	type Origin = Origin;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type Digest = generic::Digest<Log>;
	type AccountId = AccountId;
	type Lookup = Indices;
	type Header = generic::Header<BlockNumber, BlakeTwo256, Log>;
	type Event = Event;
	type Log = Log;
}

impl accounting::Trait for Runtime {
	type Event = Event;
	type CoinAmount = Balance;
	type AccountingConversions = ConversionHandler;
}

impl aura::Trait for Runtime {
	type HandleReport = aura::StakingSlasher<Runtime>;
}

impl indices::Trait for Runtime {
	type AccountIndex = AccountIndex;
	type IsDeadAccount = Balances;
	type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
	type Event = Event;
}

impl balances::Trait for Runtime {
	type Balance = Balance;
	type OnFreeBalanceZero = ((Staking, Contract), Session);
	type OnNewAccount = Indices;
	type Event = Event;
	type TransactionPayment = ();
	type DustRemoval = ();
	type TransferPayment = ();
	type Accounting = accounting::Module<Self>;
	type BalancesConversions = ConversionHandler;
}

impl consensus::Trait for Runtime {
	type Log = Log;
	type SessionKey = AuthorityId;

	// The Aura module handles offline-reports internally
	// rather than using an explicit report system.
	type InherentOfflineReport = ();
}

impl timestamp::Trait for Runtime {
	type Moment = u64;
	type OnTimestampSet = Aura;
}

impl session::Trait for Runtime {
	type ConvertAccountIdToSessionKey = ();
	type OnSessionChange = (Staking, grandpa::SyncedAuthorities<Runtime>);
	type Event = Event;
}

impl staking::Trait for Runtime {
	type Currency = balances::Module<Self>;
	type CurrencyToVote = CurrencyToVoteHandler;
	type OnRewardMinted = Treasury;
	type Event = Event;
	type Slash = ();
	type Reward = ();
}

impl democracy::Trait for Runtime {
	type Currency = balances::Module<Self>;
	type Proposal = Call;
	type Event = Event;
}

impl council::Trait for Runtime {
	type Event = Event;
	type BadPresentation = ();
	type BadReaper = ();
}

impl council::voting::Trait for Runtime {
	type Event = Event;
}

impl council::motions::Trait for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

impl treasury::Trait for Runtime {
	type Currency = balances::Module<Self>;
	type ApproveOrigin = council_motions::EnsureMembers<_4>;
	type RejectOrigin = council_motions::EnsureMembers<_2>;
	type Event = Event;
	type MintedForSpending = ();
	type ProposalRejection = ();
}

impl contract::Trait for Runtime {
	type Currency = balances::Module<Runtime>;
	type Call = Call;
	type Event = Event;
	type Gas = u64;
	type DetermineContractAddress = contract::SimpleAddressDeterminator<Runtime>;
	type ComputeDispatchFee = contract::DefaultDispatchFeeComputor<Runtime>;
	type TrieIdGenerator = contract::TrieIdFromParentCounter<Runtime>;
	type GasPayment = ();
}

impl sudo::Trait for Runtime {
	type Event = Event;
	type Proposal = Call;
}

impl grandpa::Trait for Runtime {
	type SessionKey = AuthorityId;
	type Log = Log;
	type Event = Event;
}

impl finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = grandpa::SyncedAuthorities<Runtime>;
}

// Totem impl
impl projects::Trait for Runtime {
	type Event = Event;
}

impl timekeeping::Trait for Runtime {
	type Event = Event;
	type Projects = ProjectModule;
}

impl boxkeys::Trait for Runtime {
	type Event = Event;
}

impl bonsai::Trait for Runtime {
	type Event = Event;
	type Orders = OrdersModule;
	type Projects = ProjectModule;
	type Timekeeping = TimekeepingModule;
	type BonsaiConversions = ConversionHandler;
}

impl archive::Trait for Runtime {
	type Event = Event;
	type Timekeeping = TimekeepingModule;
}

impl prefunding::Trait for Runtime {
    type Event = Event;
	type Currency = balances::Module<Self>;
	type PrefundingConversions = ConversionHandler;
    type Accounting = accounting::Module<Self>;
}

impl orders::Trait for Runtime {
	type Event = Event;
    type Accounting = accounting::Module<Self>;
	type Prefunding = PrefundingModule;
	type OrderConversions = ConversionHandler;
    type Bonsai = BonsaiModule;
}

impl funding::Trait for Runtime {
	type Event = Event;
}

impl transfer::Trait for Runtime {
	type Event = Event;
	type Currency = balances::Module<Self>;
	type TransferConversions = ConversionHandler;
	type Bonsai = BonsaiModule;
}

construct_runtime!(
	pub enum Runtime with Log(InternalLog: DigestItem<Hash, AuthorityId, AuthoritySignature>) where
		Block = Block,
		NodeBlock = node_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{default, Log(ChangesTrieRoot)},
		Accounting: accounting::{Module, Storage, Event<T>},
		Aura: aura::{Module, Inherent(Timestamp)},
		Timestamp: timestamp::{Module, Call, Storage, Config<T>, Inherent},
		Consensus: consensus::{Module, Call, Storage, Config<T>, Log(AuthoritiesChange), Inherent},
		Indices: indices,
		Balances: balances,
		Session: session,
		Staking: staking::{default, OfflineWorker},
		Democracy: democracy,
		Council: council::{Module, Call, Storage, Event<T>},
		CouncilVoting: council_voting,
		CouncilMotions: council_motions::{Module, Call, Storage, Event<T>, Origin},
		CouncilSeats: council_seats::{Config<T>},
		FinalityTracker: finality_tracker::{Module, Call, Inherent},
		Grandpa: grandpa::{Module, Call, Storage, Config<T>, Log(), Event<T>},
		Treasury: treasury,
		Contract: contract::{Module, Call, Storage, Config<T>, Event<T>},
		Sudo: sudo,
		ProjectModule: projects::{Module, Call, Storage, Event<T>},
		TimekeepingModule: timekeeping::{Module, Call, Storage, Event<T>},
		BoxKeyS: boxkeys::{Module, Call, Storage, Event<T>},
		BonsaiModule: bonsai::{Module, Call, Storage, Event<T>},
		ArchiveModule: archive::{Module, Call, Event<T>},
		OrdersModule: orders::{Module, Call, Storage, Event<T>},
        PrefundingModule: prefunding::{Module, Call, Storage, Event<T>},
        FundingModule: funding::{Module, Call, Storage, Event<T>},
        TransferModule: transfer::{Module, Call, Event<T>},
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256, Log>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedMortalCompactExtrinsic<Address, Index, Call, Signature>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Index, Call>;
/// Executive: handles dispatch to the various modules.
pub type Executive = executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Balances, AllModules>;

impl_runtime_apis! {
	impl client_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}

		fn authorities() -> Vec<AuthorityIdFor<Block>> {
			panic!("Deprecated, please use `AuthoritiesApi`.")
		}
	}

	impl client_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl block_builder_api::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			System::random_seed()
		}
	}

	impl client_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
			Executive::validate_transaction(tx)
		}
	}

	impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(number: NumberFor<Block>) {
			Executive::offchain_worker(number)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_pending_change(digest: &DigestFor<Block>)
			-> Option<ScheduledChange<NumberFor<Block>>>
		{
			for log in digest.logs.iter().filter_map(|l| match l {
				Log(InternalLog::grandpa(grandpa_signal)) => Some(grandpa_signal),
				_ => None
			}) {
				if let Some(change) = Grandpa::scrape_digest_change(log) {
					return Some(change);
				}
			}
			None
		}

		fn grandpa_forced_change(digest: &DigestFor<Block>)
			-> Option<(NumberFor<Block>, ScheduledChange<NumberFor<Block>>)>
		{
			for log in digest.logs.iter().filter_map(|l| match l {
				Log(InternalLog::grandpa(grandpa_signal)) => Some(grandpa_signal),
				_ => None
			}) {
				if let Some(change) = Grandpa::scrape_digest_forced_change(log) {
					return Some(change);
				}
			}
			None
		}

		fn grandpa_authorities() -> Vec<(AuthorityId, u64)> {
			Grandpa::grandpa_authorities()
		}
	}

	impl consensus_aura::AuraApi<Block> for Runtime {
		fn slot_duration() -> u64 {
			Aura::slot_duration()
		}
	}

	impl consensus_authorities::AuthoritiesApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityIdFor<Block>> {
			Consensus::authorities()
		}
	}
}
