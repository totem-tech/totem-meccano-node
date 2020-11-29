// Copyright 2019 Chris D'Costa
// This file is part of Totem Live Accounting.
// Author Chris D'Costa email: chris.dcosta@totemaccounting.com

// Totem is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Totem is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Totem.  If not, see <http://www.gnu.org/licenses/>.

//! The Totem Meccano Node runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// #![cfg_attr(not(feature = "std"), feature(alloc))]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

extern crate sodalite;

use client::{
    block_builder::api::{self as block_builder_api, CheckInherentsResult, InherentData},
    impl_runtime_apis, runtime_api,
};
use parity_codec:: {Encode, Decode};
#[cfg(feature = "std")]
use primitives::bytes;
use primitives::{ed25519, sr25519, OpaqueMetadata};
use rstd::prelude::*;
use runtime_primitives::{
    create_runtime_str, generic,
    traits::{self, BlakeTwo256, Block as BlockT, NumberFor, StaticLookup, Verify, Convert},
    transaction_validity::TransactionValidity,
    ApplyResult,
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use version::NativeVersion;
use version::RuntimeVersion;

// A few exports that help ease life for downstream crates.
pub use balances::Call as BalancesCall;
pub use accounting::Call as AccountingCall;
pub use consensus::Call as ConsensusCall;
#[cfg(any(feature = "std", test))]
pub use runtime_primitives::BuildStorage;
pub use runtime_primitives::{Perbill, Permill};
pub use support::{construct_runtime, StorageValue};
pub use timestamp::BlockPeriod;
pub use timestamp::Call as TimestampCall;

/// The type that is used for identifying authorities.
pub type AuthorityId = <AuthoritySignature as Verify>::Signer;

/// The type used by authorities to prove their ID.
pub type AuthoritySignature = ed25519::Signature;

/// Alias to pubkey that identifies an account on the chain.
pub type AccountId = <AccountSignature as Verify>::Signer;

/// The type used by authorities to prove their ID.
pub type AccountSignature = sr25519::Signature;

/// A hash of some data used by the chain.
pub type Hash = primitives::H256;

/// Index of a block number in the chain.
pub type BlockNumber = u64;

/// Index of an account's extrinsic in the chain.
pub type Nonce = u64;

// mod totem;
// mod accounting_traits;
// mod accounting;
// mod prefunding;
// mod prefunding_traits;
// mod orders;
// mod boxkeys;
// mod projects;
// mod timekeeping;
// mod archive;

// Test Traits
// mod marketplace;
// mod reputation_trait;
// mod simple_feedback;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core datastructures.
pub mod opaque {
    use super::*;

    /// Opaque, encoded, unchecked extrinsic.
    #[derive(PartialEq, Eq, Clone, Default, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub struct UncheckedExtrinsic(#[cfg_attr(feature = "std", serde(with = "bytes"))] pub Vec<u8>);
    #[cfg(feature = "std")]
    impl std::fmt::Debug for UncheckedExtrinsic {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(fmt, "{}", primitives::hexdisplay::HexDisplay::from(&self.0))
        }
    }
    impl traits::Extrinsic for UncheckedExtrinsic {
        fn is_signed(&self) -> Option<bool> {
            None
        }
    }
    /// Opaque block header type.
    pub type Header = generic::Header<
        BlockNumber,
        BlakeTwo256,
        generic::DigestItem<Hash, AuthorityId, AuthoritySignature>,
    >;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque session key type.
    pub type SessionKey = AuthorityId;
}

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    // node runtime name // fork risk, on change
    spec_name: create_runtime_str!("totem-meccano-node"),
    // team/implementation name
    impl_name: create_runtime_str!("totem-meccano-team"),
    // for block authoring // fork risk, on change
    authoring_version: 1,
    // spec version // fork risk, on change
    spec_version: 5,
    // incremental changes
    impl_version: 12,
    apis: RUNTIME_API_VERSIONS,
};

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
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

impl system::Trait for Runtime {
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = Indices;
    /// The index type for storing how many extrinsics an account has signed.
    type Index = Nonce;
    /// The index type for blocks.
    type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The header digest type.
    type Digest = generic::Digest<Log>;
    /// The header type.
    type Header = generic::Header<BlockNumber, BlakeTwo256, Log>;
    /// The ubiquitous event type.
    type Event = Event;
    /// The ubiquitous log type.
    type Log = Log;
    /// The ubiquitous origin type.
    type Origin = Origin;
}

impl accounting::Trait for Runtime {
	type Event = Event;
	type CoinAmount = u128;
}

impl aura::Trait for Runtime {
    type HandleReport = ();
}

impl consensus::Trait for Runtime {
    /// The identifier we use to refer to authorities.
    type SessionKey = AuthorityId;
    // The aura module handles offline-reports internally
    // rather than using an explicit report system.
    type InherentOfflineReport = ();
    /// The ubiquitous log type.
    type Log = Log;
}

impl indices::Trait for Runtime {
    /// The type for recording indexing into the account enumeration. If this ever overflows, there
    /// will be problems!
    type AccountIndex = u32;
    /// Use the standard means of resolving an index hint from an id.
    type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
    /// Determine whether an account is dead.
    type IsDeadAccount = Balances;
    /// The uniquitous event type.
    type Event = Event;
}

impl timestamp::Trait for Runtime {
    /// A timestamp: seconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
}

impl balances::Trait for Runtime {
    /// The type for recording an account's balance.
    type Balance = u128;
    /// What to do if an account's free balance gets zeroed.
    type OnFreeBalanceZero = ();
    /// What to do if a new account is created.
    type OnNewAccount = Indices;
    /// The uniquitous event type.
    type Event = Event;

    type TransactionPayment = ();
    type DustRemoval = ();
    type TransferPayment = ();

    type Accounting = accounting::Module<Self>;
}

impl sudo::Trait for Runtime {
    /// The uniquitous event type.
    type Event = Event;
    type Proposal = Call;
}

// impl projects::Trait for Runtime {
//     type Event = Event;
// }

// impl timekeeping::Trait for Runtime {
//     type Event = Event;
// }

// impl boxkeys::Trait for Runtime {
//     type Event = Event;
// }

// impl archive::Trait for Runtime {
//     type Event = Event;
// }

// impl accounting::Trait for Runtime {
//     type Event = Event;
// }

// impl prefunding::Trait for Runtime {
//     type Event = Event;
//     type Currency = balances::Module<Self>;
//     type Conversions = ConversionHandler;
//     type Accounting = AccountingModule;
// }

// impl orders::Trait for Runtime {
//     type Event = Event;
//     type Conversions = ConversionHandler;
//     type Accounting = AccountingModule;
//     type Prefunding = PrefundingModule;
// }

// impl marketplace::Trait for Runtime {
// 	type ReputationSystem = SimpleFeedback;
// 	type Event = Event;
// }

// impl simple_feedback::Trait for Runtime {
// 	type Event = Event;
// }

construct_runtime!(
	pub enum Runtime with Log(InternalLog: DigestItem<Hash, AuthorityId, AuthoritySignature>) where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{default, Log(ChangesTrieRoot)},
		Timestamp: timestamp::{Module, Call, Storage, Config<T>, Inherent},
        Consensus: consensus::{Module, Call, Storage, Config<T>, Log(AuthoritiesChange), Inherent},
        Accounting: accounting::{Module, Storage, Event<T>},
		Aura: aura::{Module},
		Indices: indices,
		Balances: balances,
		Sudo: sudo,
		// ProjectModule: projects::{Module, Call, Storage, Event<T>},
		// TimekeepingModule: timekeeping::{Module, Call, Storage, Event<T>},
		// BoxKeyS: boxkeys::{Module, Call, Storage, Event<T>},
		// ArchiveModule: archive::{Module, Call, Event<T>},
		// AccountingModule: accounting::{Module, Storage, Event<T>},
		// OrdersModule: orders::{Module, Call, Storage, Event<T>},
        // PrefundingModule: prefunding::{Module, Call, Storage, Event<T>},
        // Marketplace: marketplace::{Module, Call, Storage, Event<T>},
		// SimpleFeedback: simple_feedback::{Module, Storage, Event<T>},
	}
);

/// The type used as a helper for interpreting the sender of transactions.
type Context = system::ChainContext<Runtime>;
/// The address format for describing accounts.
type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256, Log>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedMortalCompactExtrinsic<Address, Nonce, Call, AccountSignature>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Nonce, Call>;
/// Executive: handles dispatch to the various modules.
pub type Executive = executive::Executive<Runtime, Block, Context, Balances, AllModules>;

// Implement our runtime API endpoints. This is just a bunch of proxying.
impl_runtime_apis! {
    impl runtime_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }

        fn authorities() -> Vec<AuthorityId> {
            panic!("Deprecated, please use `AuthoritiesApi`.")
        }
    }

    impl runtime_api::Metadata<Block> for Runtime {
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

    impl runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
            Executive::validate_transaction(tx)
        }
    }

    impl consensus_aura::AuraApi<Block> for Runtime {
        fn slot_duration() -> u64 {
            Aura::slot_duration()
        }
    }

    impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(n: NumberFor<Block>) {
            Executive::offchain_worker(n)
        }
    }

    impl consensus_authorities::AuthoritiesApi<Block> for Runtime {
        fn authorities() -> Vec<AuthorityId> {
            Consensus::authorities()
        }
    }
}
