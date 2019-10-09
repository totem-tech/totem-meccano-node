// Copyright 2018-2019 Parity Technologies (UK) Ltd.
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

//! Substrate chain configurations.

use primitives::{Pair, Public, crypto::UncheckedInto};
pub use node_primitives::{AccountId, Balance};
use node_runtime::{
	AuthorityDiscoveryConfig, BabeConfig, BalancesConfig, ContractsConfig, CouncilConfig, DemocracyConfig,
	ElectionsConfig, GrandpaConfig, ImOnlineConfig, IndicesConfig, SessionConfig, SessionKeys, StakerStatus,
	StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeConfig, WASM_BINARY,
};
use node_runtime::constants::{time::*, currency::*};
pub use node_runtime::GenesisConfig;
use substrate_service;
use hex_literal::hex;
use substrate_telemetry::TelemetryEndpoints;
use grandpa_primitives::{AuthorityId as GrandpaId};
use babe_primitives::{AuthorityId as BabeId};
use im_online::sr25519::{AuthorityId as ImOnlineId};
use sr_primitives::Perbill;

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// Emberic Elm testnet generator
pub fn emberic_elm_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../res/emberic-elm.json"))
}

fn staging_testnet_config_genesis() -> GenesisConfig {
	// stash, controller, session-key
	// generated with secret:
	// for i in 1 2 3 4 ; do for j in stash controller; do subkey inspect "$secret"/elm/$j/$i; done; done
	// and
	// for i in 1 2 3 4 ; do for j in session; do subkey --ed25519 inspect "$secret"//elm//$j//$i; done; done


	let initial_authorities: Vec<(AccountId, AccountId, AuthorityId)> = vec![(
		hex!["72b52eb36f57b4bae756e4f064cf2e97df80d5f9c2f06ff31206a9be8c7b371c"].unchecked_into(), // 5Ef78yxqfaxVzrFCemYcSgwVtMV85ywykhLNm5WKTsZV22HZ
		hex!["f0fae46aeb1a7ce8ca65f2bf885d09cd7f525bc00e9f6e73b5ea74402a2c4c19"].unchecked_into(), // 5HWfszmRMbzcjGmumYkkHtNJbi9y428JHgPeftVenvDgVUjh
		hex!["e29624233b2cba342750217aa1883f6ec624134dd306efd230a988e5cb37d9ed"].unchecked_into(), // 5HBoHDLMR4jPwB6BCLyd2qfYBHytFhGs8fsa1h5PzhYd3WBq
	),(
		hex!["2254035a15597c1c19968be71593d2d0131e18ae90049e49178970f583ac3e17"].unchecked_into(), // 5CqiScHtxUatcQpck1tUks51o3pSjKsdCi2CLEHvMM7tc4Qi
		hex!["eacb8edf6b05cb909a3d2bd8c6bffb13be3069ec6a69f1fa25e46103c5190267"].unchecked_into(), // 5HNZXnSgw21idbuegTC1J8Txkja97RPnnWkX68ewnrJDec2Z
		hex!["e19b6b89729a41638e57dead9c993425287d386fa4963306b63f018732843495"].unchecked_into(), // 5HAWoPYfyYFHjacy8H2MDmHra7jVrPtBfFMPgd8CadpSqotL
	),(
		hex!["fe6211db8bd436e0d1cf37398eac655833fb47497e0f72ec00ab160c88966b7e"].unchecked_into(), // 5HpF9orzkmJ9ga3yrzNS9ckifxF3tbQjadEmCEiZJQ2fPgun
		hex!["f06dd616c75cc4b2b01f325accf79b4f66a525ede0a59f48dcce2322b8798f5c"].unchecked_into(), // 5HVwyfB3LRsFXm7frEHDYyhwdpTYDRWxEqDKBYVyLi6DsPXq
		hex!["1be80f2d4513a1fbe0e5163874f729baa5498486ac3914ac3fe2e1817d7b3f44"].unchecked_into(), // 5ChJ5wjqy2HY1LZw1EuQPGQEHgaS9sFu9yDD6KRX7CzwidTN
	),(
		hex!["60779817899466dbd476a0bc3a38cc64b7774d5fb646c3d291684171e67a0743"].unchecked_into(), // 5EFByrDMMa2m9hv4jrpykXaUyqjJ9XZH81kJE4JBa1Sz2psT
		hex!["2a32622a5da54a80dc704a05f2d761c96d4748beedd83f61ca20a90f4a257678"].unchecked_into(), // 5D22qQJsLm2JUh8pEfrKahbkW21QQrHTkm4vUteei67fadLd
		hex!["f54d9f5ed217ce07c0c5faa5277a0356f8bfd884d201f9d2c9e171568e1bf077"].unchecked_into(), // 5HcLeWrsfL9RuGp94pn1PeFxP7D1587TTEZzFYgFhKCPZLYh
	)];
	// generated with secret: subkey inspect "$secret"/elm
	let endowed_accounts: Vec<AccountId> = vec![
		hex!["c224ccba63292331623bbf06a55f46607824c2580071a80a17c53cab2f999e2f"].unchecked_into(), //5GTG5We6twtoF6S4kUXJ77rWBsHBoHLS3JVf5KvvnxKdGQZr
	];

	const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
	const STASH: Balance = 100 * DOLLARS;

	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().cloned()
				.map(|k| (k, ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
			vesting: vec![],
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.iter().cloned()
				.chain(initial_authorities.iter().map(|x| x.0.clone()))
				.collect::<Vec<_>>(),
		}),
		session: Some(SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone()))
			}).collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			validator_count: 7,
			minimum_validator_count: 4,
			stakers: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator)
			}).collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			.. Default::default()
		}),
		democracy: Some(DemocracyConfig::default()),
		collective_Instance1: Some(CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		collective_Instance2: Some(TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		elections: Some(ElectionsConfig {
			members: vec![],
			presentation_duration: 1 * DAYS,
			term_duration: 28 * DAYS,
			desired_seats: 0,
		}),
		contracts: Some(ContractsConfig {
			current_schedule: Default::default(),
			gas_price: 1 * MILLICENTS,
		}),
		sudo: Some(SudoConfig {
			key: endowed_accounts[0].clone(),
		}),
		babe: Some(BabeConfig {
			authorities: vec![],
		}),
		im_online: Some(ImOnlineConfig {
			keys: vec![],
		}),
		authority_discovery: Some(AuthorityDiscoveryConfig{
			keys: vec![],
		}),
		grandpa: Some(GrandpaConfig {
			authorities: vec![],
		}),
		membership_Instance1: Some(Default::default()),
	}
}

/// Staging testnet config.
pub fn staging_testnet_config() -> ChainSpec {
	let boot_nodes = vec![];
	ChainSpec::from_genesis(
		"Staging Testnet",
		"staging_testnet",
		staging_testnet_config_genesis,
		boot_nodes,
		Some(TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])),
		None,
		None,
		None,
	)
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}


/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, GrandpaId, BabeId, ImOnlineId) {
	(
		get_from_seed::<AccountId>(&format!("{}//stash", seed)),
		get_from_seed::<AccountId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<ImOnlineId>(seed),
	)
}

/// Helper function to create GenesisConfig for testing
pub fn testnet_genesis(
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId, ImOnlineId)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
	enable_println: bool,
) -> GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(|| {
		vec![
			get_from_seed::<AccountId>("Alice"),
			get_from_seed::<AccountId>("Bob"),
			get_from_seed::<AccountId>("Charlie"),
			get_from_seed::<AccountId>("Dave"),
			get_from_seed::<AccountId>("Eve"),
			get_from_seed::<AccountId>("Ferdie"),
			get_from_seed::<AccountId>("Alice//stash"),
			get_from_seed::<AccountId>("Bob//stash"),
			get_from_seed::<AccountId>("Charlie//stash"),
			get_from_seed::<AccountId>("Dave//stash"),
			get_from_seed::<AccountId>("Eve//stash"),
			get_from_seed::<AccountId>("Ferdie//stash"),
		]
	});

	const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
	const STASH: Balance = 100 * DOLLARS;

	let desired_seats = (endowed_accounts.len() / 2 - initial_authorities.len()) as u32;

	let mut contract_config = ContractConfig {
		transaction_base_fee: 1,
		transaction_byte_fee: 0,
		transfer_fee: 0,
		creation_fee: 0,
		contract_fee: 21,
		call_base_fee: 135,
		create_base_fee: 175,
		gas_price: 1,
		max_depth: 1024,
		block_gas_limit: 10_000_000,
		current_schedule: Default::default(),
	};
	// this should only be enabled on development chains
	contract_config.current_schedule.enable_println = enable_println;

	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
			vesting: vec![],
		}),
		session: Some(SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone()))
			}).collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			minimum_validator_count: 1,
			validator_count: 2,
			stakers: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator)
			}).collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			.. Default::default()
		}),
		democracy: Some(DemocracyConfig::default()),
		collective_Instance1: Some(CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		collective_Instance2: Some(TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		elections: Some(ElectionsConfig {
			members: endowed_accounts.iter()
				.filter(|&endowed| initial_authorities.iter().find(|&(_, controller, ..)| controller == endowed).is_none())
				.map(|a| (a.clone(), 1000000)).collect(),
			presentation_duration: 10,
			term_duration: 1000000,
			desired_seats: desired_seats,
		}),
		contracts: Some(ContractsConfig {
			current_schedule: contracts::Schedule {
				enable_println, // this should only be enabled on development chains
				..Default::default()
			},
			gas_price: 1 * MILLICENTS,
		}),
		sudo: Some(SudoConfig {
			key: root_key,
		}),
		babe: Some(BabeConfig {
			authorities: vec![],
		}),
		contract: Some(contract_config),
		sudo: Some(SudoConfig {
			key: root_key,
		}),
		grandpa: Some(GrandpaConfig {
			authorities: vec![],
		}),
		membership_Instance1: Some(Default::default()),
	}
}

fn development_config_genesis() -> GenesisConfig {
	testnet_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
		],
		get_from_seed::<AccountId>("Alice"),
		None,
		true,
	)
}

/// Development config (single validator Alice)
pub fn development_config() -> ChainSpec {
	ChainSpec::from_genesis("Development", "dev", development_config_genesis, vec![], None, None, None, None)
}

fn local_testnet_genesis() -> GenesisConfig {
	testnet_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
		],
		get_from_seed::<AccountId>("Alice"),
		None,
		false,
	)
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn local_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis("Local Testnet", "local_testnet", local_testnet_genesis, vec![], None, None, None, None)
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use crate::service::{new_full, new_light};
	use service_test;

	fn local_testnet_genesis_instant_single() -> GenesisConfig {
		testnet_genesis(
			vec![
				get_authority_keys_from_seed("Alice"),
			],
			get_from_seed::<AccountId>("Alice"),
			None,
			false,
		)
	}

	/// Local testnet config (single validator - Alice)
	pub fn integration_test_config_with_single_authority() -> ChainSpec {
		ChainSpec::from_genesis(
			"Integration Test",
			"test",
			local_testnet_genesis_instant_single,
			vec![],
			None,
			None,
			None,
			None,
		)
	}

	/// Local testnet config (multivalidator Alice + Bob)
	pub fn integration_test_config_with_two_authorities() -> ChainSpec {
		ChainSpec::from_genesis(
			"Integration Test",
			"test",
			local_testnet_genesis,
			vec![],
			None,
			None,
			None,
			None,
		)
	}

	#[test]
	#[ignore]
	fn test_connectivity() {
		service_test::connectivity(
			integration_test_config_with_two_authorities(),
			|config| new_full(config),
			|config| new_light(config),
		);
	}
}
