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

#![allow(missing_docs)]

#[cfg(test)]
mod block_import;
#[cfg(test)]
mod sync;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::config::build_multiaddr;
use log::trace;
use crate::chain::FinalityProofProvider;
use client::{
	self, ClientInfo, BlockchainEvents, BlockImportNotification, FinalityNotifications,
	FinalityNotification, LongestChain
};
use client::error::Result as ClientResult;
use client::block_builder::BlockBuilder;
use crate::config::ProtocolConfig;
use consensus::import_queue::{BasicQueue, ImportQueue, IncomingBlock};
use consensus::import_queue::{Link, SharedBlockImport, SharedJustificationImport, Verifier};
use consensus::{Error as ConsensusError, ErrorKind as ConsensusErrorKind};
use consensus::{BlockOrigin, ForkChoiceStrategy, ImportBlock, JustificationImport};
use crate::consensus_gossip::ConsensusGossip;
use crossbeam_channel::{Sender, RecvError};
use futures::Future;
use futures::sync::{mpsc, oneshot};
use crate::message::Message;
use network_libp2p::PeerId;
use parity_codec::Encode;
use parking_lot::{Mutex, RwLock};
use primitives::{H256, ed25519::Public as AuthorityId};
use crate::protocol::{ConnectedPeer, Context, FromNetworkMsg, Protocol, ProtocolMsg};
use runtime_primitives::generic::BlockId;
use runtime_primitives::traits::{AuthorityIdFor, Block as BlockT, Digest, DigestItem, Header, NumberFor};
use runtime_primitives::{Justification, ConsensusEngineId};
use crate::service::{network_channel, NetworkChan, NetworkLink, NetworkMsg, NetworkPort, TransactionPool};
use crate::specialization::NetworkSpecialization;
use test_client::{self, AccountKeyring};

pub use test_client::runtime::{Block, Extrinsic, Hash, Transfer};
pub use test_client::{TestClient, TestClientBuilder, TestClientBuilderExt};

type AuthorityId = babe_primitives::AuthorityId;

#[cfg(any(test, feature = "test-helpers"))]
/// A Verifier that accepts all blocks and passes them on with the configured
/// finality to be imported.
#[derive(Clone)]
pub struct PassThroughVerifier(pub bool);

#[cfg(any(test, feature = "test-helpers"))]
/// This `Verifier` accepts all data as valid.
impl<B: BlockT> Verifier<B> for PassThroughVerifier {
	fn verify(
		&mut self,
		origin: BlockOrigin,
		header: B::Header,
		justification: Option<Justification>,
		body: Option<Vec<B::Extrinsic>>
	) -> Result<(BlockImportParams<B>, Option<Vec<(CacheKeyId, Vec<u8>)>>), String> {
		let maybe_keys = header.digest()
			.log(|l| l.try_as_raw(OpaqueDigestItemId::Consensus(b"aura"))
				.or_else(|| l.try_as_raw(OpaqueDigestItemId::Consensus(b"babe")))
			)
			.map(|blob| vec![(well_known_cache_keys::AUTHORITIES, blob.to_vec())]);

		Ok((BlockImportParams {
			origin,
			header,
			body,
			finalized: self.0,
			justification,
			post_digests: vec![],
			auxiliary: Vec::new(),
			fork_choice: ForkChoiceStrategy::LongestChain,
		}, maybe_keys))
	}
}

/// The test specialization.
#[derive(Clone)]
pub struct DummySpecialization;

impl NetworkSpecialization<Block> for DummySpecialization {
	fn status(&self) -> Vec<u8> {
		vec![]
	}

	fn on_connect(
		&mut self,
		_ctx: &mut dyn Context<Block>,
		_peer_id: PeerId,
		_status: crate::message::Status<Block>
	) {}

	fn on_disconnect(&mut self, _ctx: &mut dyn Context<Block>, _peer_id: PeerId) {}

	fn on_message(
		&mut self,
		_ctx: &mut dyn Context<Block>,
		_peer_id: PeerId,
		_message: Vec<u8>,
	) {}

	fn on_event(
		&mut self,
		_event: crate::specialization::Event
	) {}
}

pub type PeersFullClient =
	client::Client<test_client::Backend, test_client::Executor, Block, test_client::runtime::RuntimeApi>;
pub type PeersLightClient =
	client::Client<test_client::LightBackend, test_client::LightExecutor, Block, test_client::runtime::RuntimeApi>;

#[derive(Clone)]
/// A Link that can wait for a block to have been imported.
pub struct TestLink<S: NetworkSpecialization<Block>> {
	link: NetworkLink<Block, S>,
	network_to_protocol_sender: Sender<FromNetworkMsg<Block>>,
}

impl<S: NetworkSpecialization<Block> + Clone> TestLink<S> {
	fn new(
		protocol_sender: Sender<ProtocolMsg<Block, S>>,
		network_to_protocol_sender: Sender<FromNetworkMsg<Block>>,
		network_sender: NetworkChan<Block>
	) -> TestLink<S> {
		TestLink {
			network_to_protocol_sender,
			link: NetworkLink {
				protocol_sender,
				network_sender,
			}
		}
	}
}

impl<S: NetworkSpecialization<Block> + Clone> Link<Block> for TestLink<S> {
	fn block_imported(&self, hash: &Hash, number: NumberFor<Block>) {
		self.link.block_imported(hash, number);
	}

	fn blocks_processed(&self, processed_blocks: Vec<Hash>, has_error: bool) {
		self.link.blocks_processed(processed_blocks, has_error);
	}

	fn justification_imported(&self, who: PeerId, hash: &Hash, number:NumberFor<Block>, success: bool) {
		self.link.justification_imported(who, hash, number, success);
	}

	fn request_justification(&self, hash: &Hash, number: NumberFor<Block>) {
		self.link.request_justification(hash, number);
	}

	fn useless_peer(&self, who: PeerId, reason: &str) {
		self.link.useless_peer(who, reason);
	}

	fn note_useless_and_restart_sync(&self, who: PeerId, reason: &str) {
		self.link.note_useless_and_restart_sync(who, reason);
	}

	fn restart(&self) {
		self.link.restart();
	}

	fn synchronized(&self) {
		let _ = self.network_to_protocol_sender.send(FromNetworkMsg::Synchronize);
	}
}

pub struct Peer<D, S: NetworkSpecialization<Block> + Clone> {
	pub is_offline: Arc<AtomicBool>,
	pub is_major_syncing: Arc<AtomicBool>,
	pub peers: Arc<RwLock<HashMap<PeerId, ConnectedPeer<Block>>>>,
	pub peer_id: PeerId,
	client: Arc<PeersClient>,
	net_proto_channel: ProtocolChannel<S>,
	pub import_queue: Box<BasicQueue<Block>>,
	pub data: D,
	best_hash: Mutex<Option<H256>>,
	finalized_hash: Mutex<Option<H256>>,
}

type MessageFilter = Fn(&NetworkMsg<Block>) -> bool;

struct ProtocolChannel<S: NetworkSpecialization<Block>> {
	buffered_messages: Mutex<VecDeque<NetworkMsg<Block>>>,
	network_to_protocol_sender: Sender<FromNetworkMsg<Block>>,
	client_to_protocol_sender: Sender<ProtocolMsg<Block, S>>,
	protocol_to_network_receiver: NetworkPort<Block>,
}

impl<S: NetworkSpecialization<Block>> ProtocolChannel<S> {
	/// Create new buffered network port.
	pub fn new(
		network_to_protocol_sender: Sender<FromNetworkMsg<Block>>,
		client_to_protocol_sender: Sender<ProtocolMsg<Block, S>>,
		protocol_to_network_receiver: NetworkPort<Block>,
	) -> Self {
		ProtocolChannel {
			buffered_messages: Mutex::new(VecDeque::new()),
			network_to_protocol_sender,
			client_to_protocol_sender,
			protocol_to_network_receiver,
		}
	}

	/// Send message from network to protocol.
	pub fn send_from_net(&self, message: FromNetworkMsg<Block>) {
		let _ = self.network_to_protocol_sender.send(message);

		let _ = self.network_to_protocol_sender.send(FromNetworkMsg::Synchronize);
		let _ = self.wait_sync();
	}

	/// Send message from client to protocol.
	pub fn send_from_client(&self, message: ProtocolMsg<Block, S>) {
		let _ = self.client_to_protocol_sender.send(message);

		let _ = self.client_to_protocol_sender.send(ProtocolMsg::Synchronize);
		let _ = self.wait_sync();
	}

	/// Wait until synchronization response is generated by the protocol.
	pub fn wait_sync(&self) -> Result<(), RecvError> {
		loop {
			match self.protocol_to_network_receiver.receiver().recv() {
				Ok(NetworkMsg::Synchronized) => return Ok(()),
				Err(error) => return Err(error),
				Ok(msg) => self.buffered_messages.lock().push_back(msg),
			}
		}
	}

	/// Produce the next pending message to send to another peer.
	fn pending_message(&self, message_filter: &MessageFilter) -> Option<NetworkMsg<Block>> {
		if let Some(message) = self.buffered_message(message_filter) {
			return Some(message);
		}

		while let Some(message) = self.channel_message() {
			if message_filter(&message) {
				return Some(message)
			} else {
				self.buffered_messages.lock().push_back(message);
			}
		}

		None
	}

	/// Whether this peer is done syncing (has no messages to send).
	fn is_done(&self) -> bool {
		self.buffered_messages.lock().is_empty()
			&& self.protocol_to_network_receiver.receiver().is_empty()
	}

	/// Return oldest buffered message if it exists.
	fn buffered_message(&self, message_filter: &MessageFilter) -> Option<NetworkMsg<Block>> {
		let mut buffered_messages = self.buffered_messages.lock();
		for i in 0..buffered_messages.len() {
			if message_filter(&buffered_messages[i]) {
				return buffered_messages.remove(i);
			}
		}

		None
	}

	/// Receive message from the channel.
	fn channel_message(&self) -> Option<NetworkMsg<Block>> {
		self.protocol_to_network_receiver.receiver().try_recv().ok()
	}
}

impl<D, S: NetworkSpecialization<Block> + Clone> Peer<D, S> {
	fn new(
		is_offline: Arc<AtomicBool>,
		is_major_syncing: Arc<AtomicBool>,
		peers: Arc<RwLock<HashMap<PeerId, ConnectedPeer<Block>>>>,
		client: Arc<PeersClient>,
		import_queue: Box<BasicQueue<Block>>,
		network_to_protocol_sender: Sender<FromNetworkMsg<Block>>,
		protocol_sender: Sender<ProtocolMsg<Block, S>>,
		network_sender: NetworkChan<Block>,
		network_port: NetworkPort<Block>,
		data: D,
	) -> Self {
		let net_proto_channel = ProtocolChannel::new(
			network_to_protocol_sender.clone(),
			protocol_sender.clone(),
			network_port,
		);
		let network_link = TestLink::new(
			protocol_sender.clone(),
			network_to_protocol_sender.clone(),
			network_sender.clone(),
		);
		import_queue.start(Box::new(network_link)).expect("Test ImportQueue always starts");
		Peer {
			is_offline,
			is_major_syncing,
			peers,
			peer_id: PeerId::random(),
			client,
			import_queue,
			net_proto_channel,
			data,
			best_hash: Mutex::new(None),
			finalized_hash: Mutex::new(None),
		}
	}
	/// Called after blockchain has been populated to updated current state.
	fn start(&self) {
		// Update the sync state to the latest chain state.
		let info = self.client.info().expect("In-mem client does not fail");
		let header = self
			.client
			.header(&BlockId::Hash(info.chain.best_hash))
			.unwrap()
			.unwrap();
		self.net_proto_channel.send_from_client(ProtocolMsg::BlockImported(info.chain.best_hash, header));
	}

	pub fn on_block_imported(
		&self,
		hash: <Block as BlockT>::Hash,
		header: &<Block as BlockT>::Header,
	) {
		self.net_proto_channel.send_from_client(ProtocolMsg::BlockImported(hash, header.clone()));
	}

	// SyncOracle: are we connected to any peer?
	#[cfg(test)]
	fn is_offline(&self) -> bool {
		self.is_offline.load(::std::sync::atomic::Ordering::Relaxed)
	}

	// SyncOracle: are we in the process of catching-up with the chain?
	#[cfg(test)]
	fn is_major_syncing(&self) -> bool {
		self.is_major_syncing.load(::std::sync::atomic::Ordering::Relaxed)
	}

	/// Called on connection to other indicated peer.
	fn on_connect(&self, other: &Self) {
		self.net_proto_channel.send_from_net(FromNetworkMsg::PeerConnected(other.peer_id.clone(), String::new()));
	}

	/// Called on disconnect from other indicated peer.
	fn on_disconnect(&self, other: &Self) {
		self.net_proto_channel.send_from_net(FromNetworkMsg::PeerDisconnected(other.peer_id.clone(), String::new()));
	}

	/// Receive a message from another peer. Return a set of peers to disconnect.
	fn receive_message(&self, from: &PeerId, msg: Message<Block>) {
		self.net_proto_channel.send_from_net(FromNetworkMsg::CustomMessage(from.clone(), msg));
	}

	/// Produce the next pending message to send to another peer.
	fn pending_message(&self, message_filter: &MessageFilter) -> Option<NetworkMsg<Block>> {
		self.net_proto_channel.pending_message(message_filter)
	}

	/// Whether this peer is done syncing (has no messages to send).
	fn is_done(&self) -> bool {
		self.net_proto_channel.is_done()
	}

	/// Synchronize with import queue.
	fn import_queue_sync(&self) {
		self.import_queue.synchronize();
		let _ = self.net_proto_channel.wait_sync();
	}

	/// Execute a "sync step". This is called for each peer after it sends a packet.
	fn sync_step(&self) {
		self.net_proto_channel.send_from_client(ProtocolMsg::Tick);
	}

	pub fn finality_notification_stream(&self) -> FinalityNotifications<Block> {
		match *self {
			PeersClient::Full(ref client, ref _backend) => client.finality_notification_stream(),
			PeersClient::Light(ref client, ref _backend) => client.finality_notification_stream(),
		}

		let header = self.client.header(&BlockId::Hash(info.chain.best_hash)).unwrap().unwrap();
		self.net_proto_channel.send_from_client(ProtocolMsg::BlockImported(info.chain.best_hash, header));
		*best_hash = Some(info.chain.best_hash);
	}

	pub fn finalize_block(
		&self,
		id: BlockId<Block>,
		justification: Option<Justification>,
		notify: bool
	) -> ClientResult<()> {
		match *self {
			PeersClient::Full(ref client, ref _backend) => client.finalize_block(id, justification, notify),
			PeersClient::Light(ref client, ref _backend) => client.finalize_block(id, justification, notify),
		}

		let header = self.client.header(&BlockId::Hash(info.chain.finalized_hash)).unwrap().unwrap();
		self.net_proto_channel.send_from_client(ProtocolMsg::BlockFinalized(info.chain.finalized_hash, header.clone()));
		*finalized_hash = Some(info.chain.finalized_hash);
	}
}

	/// Restart sync for a peer.
	fn restart_sync(&self) {
		self.net_proto_channel.send_from_client(ProtocolMsg::Abort);
	}

	/// Push a message into the gossip network and relay to peers.
	/// `TestNet::sync_step` needs to be called to ensure it's propagated.
	pub fn gossip_message(
		&self,
		topic: <Block as BlockT>::Hash,
		engine_id: ConsensusEngineId,
		data: Vec<u8>,
		force: bool,
	) {
		self.net_proto_channel.send_from_client(
			ProtocolMsg::GossipConsensusMessage(topic, engine_id, data, force),
		);
	}

	// Returns a clone of the local SelectChain, only available on full nodes
	pub fn select_chain(&self) -> Option<LongestChain<test_client::Backend, Block>> {
		self.select_chain.clone()
	}

	/// Returns the number of peers we're connected to.
	pub fn num_peers(&self) -> usize {
		self.network.num_connected_peers()
	}

	/// Execute a closure with the consensus gossip.
	pub fn with_gossip<F>(&self, f: F)
		where F: FnOnce(&mut ConsensusGossip<Block>, &mut Context<Block>) + Send + 'static
	{
		self.net_proto_channel.send_from_client(ProtocolMsg::ExecuteWithGossip(Box::new(f)));
	}

	/// Announce a block to peers.
	pub fn announce_block(&self, block: Hash) {
		self.net_proto_channel.send_from_client(ProtocolMsg::AnnounceBlock(block));
	}

	/// Request a justification for the given block.
	#[cfg(test)]
	fn request_justification(&self, hash: &::primitives::H256, number: NumberFor<Block>) {
		self.net_proto_channel.send_from_client(ProtocolMsg::RequestJustification(hash.clone(), number));
	}

	/// Add blocks to the peer -- edit the block before adding
	pub fn generate_blocks<F>(&mut self, count: usize, origin: BlockOrigin, edit_block: F) -> H256
		where F: FnMut(BlockBuilder<Block, PeersFullClient>) -> Block
	{
		let best_hash = self.client.info().chain.best_hash;
		self.generate_blocks_at(BlockId::Hash(best_hash), count, origin, edit_block)
	}

	/// Add blocks to the peer -- edit the block before adding. The chain will
	/// start at the given block iD.
	fn generate_blocks_at<F>(
		&mut self,
		at: BlockId<Block>,
		count: usize,
		origin: BlockOrigin,
		mut edit_block: F
	) -> H256 where F: FnMut(BlockBuilder<Block, PeersFullClient>) -> Block {
		let full_client = self.client.as_full().expect("blocks could only be generated by full clients");
		let mut at = full_client.header(&at).unwrap().unwrap().hash();
		for _  in 0..count {
			let builder = full_client.new_block_at(&BlockId::Hash(at), Default::default()
			).unwrap();
			let block = edit_block(builder);
			let hash = block.header.hash();
			trace!(
				target: "test_network",
				"Generating {}, (#{}, parent={})",
				hash,
				block.header.number,
				block.header.parent_hash
			);
			let header = block.header.clone();
			at = hash;
			self.import_queue.import_blocks(
				origin,
				vec![IncomingBlock {
					origin: None,
					hash,
					header: Some(header),
					body: Some(block.extrinsics),
					justification: None,
				}],
			);

			// make sure block import has completed
			self.import_queue_sync();
		}

		at
	}

	/// Push blocks to the peer (simplified: with or without a TX)
	pub fn push_blocks(&mut self, count: usize, with_tx: bool) -> H256 {
		let best_hash = self.client.info().chain.best_hash;
		self.push_blocks_at(BlockId::Hash(best_hash), count, with_tx)
	}

	/// Push blocks to the peer (simplified: with or without a TX) starting from
	/// given hash.
	pub fn push_blocks_at(&mut self, at: BlockId<Block>, count: usize, with_tx: bool) -> H256 {
		let mut nonce = 0;
		if with_tx {
			self.generate_blocks_at(at, count, BlockOrigin::File, |mut builder| {
				let transfer = Transfer {
					from: AccountKeyring::Alice.into(),
					to: AccountKeyring::Alice.into(),
					amount: 1,
					nonce,
				};
				builder.push(transfer.into_signed_tx()).unwrap();
				nonce = nonce + 1;
				builder.bake().unwrap()
			})
		} else {
			self.generate_blocks_at(at, count, BlockOrigin::File, |builder| builder.bake().unwrap())
		}
	}

	pub fn push_authorities_change_block(&mut self, new_authorities: Vec<AuthorityId>) -> H256 {
		self.generate_blocks(1, BlockOrigin::File, |mut builder| {
			builder.push(Extrinsic::AuthoritiesChange(new_authorities.clone())).unwrap();
			builder.bake().unwrap()
		})
	}

	/// Get a reference to the client.
	pub fn client(&self) -> &PeersClient {
		&self.client
	}

	/// Get a reference to the network service.
	pub fn network_service(&self) -> &Arc<NetworkService<Block, S, <Block as BlockT>::Hash>> {
		&self.network.service()
	}

	/// Test helper to compare the blockchain state of multiple (networked)
	/// clients.
	/// Potentially costly, as it creates in-memory copies of both blockchains in order
	/// to compare them. If you have easier/softer checks that are sufficient, e.g.
	/// by using .info(), you should probably use it instead of this.
	pub fn blockchain_canon_equals(&self, other: &Self) -> bool {
		if let (Some(mine), Some(others)) = (self.backend.clone(), other.backend.clone()) {
			mine.as_in_memory().blockchain()
				.canon_equals_to(others.as_in_memory().blockchain())
		} else {
			false
		}
	}

	/// Count the current number of known blocks. Note that:
	///  1. this might be expensive as it creates an in-memory-copy of the chain
	///     to count the blocks, thus if you have a different way of testing this
	///     (e.g. `info.best_hash`) - use that.
	///  2. This is not always increasing nor accurate, as the
	///     orphaned and proven-to-never-finalized blocks may be pruned at any time.
	///     Therefore, this number can drop again.
	pub fn blocks_count(&self) -> usize {
		self.backend.as_ref().map(
			|backend| backend.as_in_memory().blockchain().blocks_count()
		).unwrap_or(0)
	}
}

pub struct EmptyTransactionPool;

impl TransactionPool<Hash, Block> for EmptyTransactionPool {
	fn transactions(&self) -> Vec<(Hash, Extrinsic)> {
		Vec::new()
	}

	fn import(&self, _transaction: &Extrinsic) -> Option<Hash> {
		None
	}

	fn on_broadcasted(&self, _: HashMap<Hash, Vec<String>>) {}
}

pub trait SpecializationFactory {
	fn create() -> Self;
}

impl SpecializationFactory for DummySpecialization {
	fn create() -> DummySpecialization {
		DummySpecialization
	}
}

/// Implements `BlockImport` on an `Arc<Mutex<impl BlockImport>>`. Used internally. Necessary to overcome the way the
/// `TestNet` trait is designed, more specifically `make_block_import` returning a `Box<BlockImport>` makes it
/// impossible to clone the underlying object.
struct BlockImportAdapter<T: ?Sized>(Arc<Mutex<Box<T>>>);

impl<T: ?Sized> Clone for BlockImportAdapter<T> {
	fn clone(&self) -> Self {
		BlockImportAdapter(self.0.clone())
	}
}

impl<T: ?Sized + BlockImport<Block>> BlockImport<Block> for BlockImportAdapter<T> {
	type Error = T::Error;

	fn check_block(&mut self, hash: Hash, parent_hash: Hash) -> Result<ImportResult, Self::Error> {
		self.0.lock().check_block(hash, parent_hash)
	}

	fn import_block(
		&mut self,
		block: BlockImportParams<Block>,
		cache: HashMap<well_known_cache_keys::Id, Vec<u8>>,
	) -> Result<ImportResult, Self::Error> {
		self.0.lock().import_block(block, cache)
	}
}

/// Implements `Verifier` on an `Arc<Mutex<impl Verifier>>`. Used internally.
struct VerifierAdapter<T: ?Sized>(Arc<Mutex<Box<T>>>);

impl<T: ?Sized> Clone for VerifierAdapter<T> {
	fn clone(&self) -> Self {
		VerifierAdapter(self.0.clone())
	}
}

impl<B: BlockT, T: ?Sized + Verifier<B>> Verifier<B> for VerifierAdapter<T> {
	fn verify(
		&mut self,
		origin: BlockOrigin,
		header: B::Header,
		justification: Option<Justification>,
		body: Option<Vec<B::Extrinsic>>
	) -> Result<(BlockImportParams<B>, Option<Vec<(CacheKeyId, Vec<u8>)>>), String> {
		self.0.lock().verify(origin, header, justification, body)
	}
}

pub trait TestNetFactory: Sized {
	type Specialization: NetworkSpecialization<Block> + SpecializationFactory;
	type Verifier: 'static + Verifier<Block>;
	type PeerData: Default;

	/// These two need to be implemented!
	fn from_config(config: &ProtocolConfig) -> Self;
	fn make_verifier(&self, client: PeersClient, config: &ProtocolConfig) -> Self::Verifier;

	/// Get reference to peer.
	fn peer(&mut self, i: usize) -> &mut Peer<Self::PeerData, Self::Specialization>;
	fn peers(&self) -> &Vec<Peer<Self::PeerData, Self::Specialization>>;
	fn mut_peers<F: FnOnce(&mut Vec<Peer<Self::PeerData, Self::Specialization>>)>(&mut self, closure: F);

	/// Get custom block import handle for fresh client, along with peer data.
	fn make_block_import(&self, client: PeersClient)
		-> (
			BoxBlockImport<Block>,
			Option<BoxJustificationImport<Block>>,
			Option<BoxFinalityProofImport<Block>>,
			Option<BoxFinalityProofRequestBuilder<Block>>,
			Self::PeerData,
		)
	{
		(client.as_block_import(), None, None, None, Default::default())
	}

	/// Get finality proof provider (if supported).
	fn make_finality_proof_provider(&self, _client: PeersClient) -> Option<Arc<dyn FinalityProofProvider<Block>>> {
		None
	}

	fn default_config() -> ProtocolConfig {
		ProtocolConfig::default()
	}

	/// Create new test network with this many peers.
	fn new(n: usize) -> Self {
		trace!(target: "test_network", "Creating test network");
		let config = Self::default_config();
		let mut net = Self::from_config(&config);

		for i in 0..n {
			trace!(target: "test_network", "Adding peer {}", i);
			net.add_full_peer(&config);
		}
		net
	}

	/// Add a full peer.
	fn add_full_peer(&mut self, config: &ProtocolConfig) {
		let test_client_builder = TestClientBuilder::with_default_backend();
		let backend = test_client_builder.backend();
		let (c, longest_chain) = test_client_builder.build_with_longest_chain();
		let client = Arc::new(c);
		let verifier = self.make_verifier(PeersClient::Full(client.clone(), backend.clone()), config);
		let verifier = VerifierAdapter(Arc::new(Mutex::new(Box::new(verifier) as Box<_>)));
		let (block_import, justification_import, finality_proof_import, finality_proof_request_builder, data)
			= self.make_block_import(PeersClient::Full(client.clone(), backend.clone()));
		let block_import = BlockImportAdapter(Arc::new(Mutex::new(block_import)));

		let import_queue = Box::new(BasicQueue::new(
			verifier.clone(),
			Box::new(block_import.clone()),
			justification_import,
			finality_proof_import,
		));

		let listen_addr = build_multiaddr![Memory(rand::random::<u64>())];

		let network = NetworkWorker::new(crate::config::Params {
			roles: config.roles,
			network_config: NetworkConfiguration {
				listen_addresses: vec![listen_addr.clone()],
				transport: TransportConfig::MemoryOnly,
				..NetworkConfiguration::default()
			},
			chain: client.clone(),
			finality_proof_provider: self.make_finality_proof_provider(PeersClient::Full(client.clone(), backend.clone())),
			finality_proof_request_builder,
			on_demand: None,
			transaction_pool: Arc::new(EmptyTransactionPool),
			protocol_id: ProtocolId::from(&b"test-protocol-name"[..]),
			import_queue,
			specialization: self::SpecializationFactory::create(),
		}).unwrap();

	/// Start network.
	fn start(&mut self) {
		if self.started() {
			return;
		}
		for peer in self.peers() {
			peer.start();
			for client in self.peers() {
				if peer.peer_id != client.peer_id {
					peer.on_connect(client);
				}
			}
		}

		loop {
			// we only deliver Status messages during start
			let need_continue = self.route_single(true, None, &|msg| match *msg {
				NetworkMsg::Outgoing(_, crate::message::generic::Message::Status(_)) => true,
				NetworkMsg::Outgoing(_, _) => false,
				NetworkMsg::ReportPeer(_, _) | NetworkMsg::Synchronized => true,
			});
			if !need_continue {
				break;
			}
		}

		self.set_started(true);
	}

	/// Do single round of message routing: single message from every peer is routed.
	fn route_single(
		&mut self,
		disconnect: bool,
		disconnected: Option<HashSet<usize>>,
		message_filter: &MessageFilter,
	) -> bool {
		let mut had_messages = false;
		let mut to_disconnect = HashSet::new();
		let peers = self.peers();
		for peer in peers {
			if let Some(message) = peer.pending_message(message_filter) {
				match message {
					NetworkMsg::Outgoing(recipient_id, packet) => {
						had_messages = true;

						let sender_pos = peers.iter().position(|p| p.peer_id == peer.peer_id).unwrap();
						let recipient_pos = peers.iter().position(|p| p.peer_id == recipient_id).unwrap();
						if disconnect {
							if let Some(ref disconnected) = disconnected {
								let mut current = HashSet::new();
								current.insert(sender_pos);
								current.insert(recipient_pos);
								// Not routing message between "disconnected" nodes.
								if disconnected.is_subset(&current) {
									continue;
								}
							}
						}

						peers[recipient_pos].receive_message(&peer.peer_id, packet);
					},
					NetworkMsg::ReportPeer(who, _) => {
						if disconnect {
							to_disconnect.insert(who);
						}
					},
					_ => (),
				}
			}
		}

		for d in to_disconnect {
			if let Some(d) = peers.iter().find(|p| p.peer_id == d) {
				for peer in 0..peers.len() {
					peers[peer].on_disconnect(d);
				}
			}
		}

		// make sure that the protocol(s) has processed all messages that have been queued
		self.peers().iter().for_each(|peer| peer.import_queue_sync());

		had_messages
	}

	/// Send block import notifications for all peers.
	fn send_import_notifications(&mut self) {
		self.peers().iter().for_each(|peer| peer.send_import_notifications())
	}

	/// Send block finalization notifications for all peers.
	fn send_finality_notifications(&mut self) {
		self.peers().iter().for_each(|peer| peer.send_finality_notifications())
	}

			let imported_blocks_stream = Box::new(client.import_notification_stream()
				.map(|v| Ok::<_, ()>(v)).compat().fuse());
			let finality_notification_stream = Box::new(client.finality_notification_stream()
				.map(|v| Ok::<_, ()>(v)).compat().fuse());

			peers.push(Peer {
				data,
				verifier,
				select_chain: None,
				backend: None,
				block_import: Box::new(block_import),
				client: PeersClient::Light(client, backend),
				imported_blocks_stream,
				finality_notification_stream,
				network,
			});
		});
	}

	/// Perform synchronization until complete, if provided the
	/// given nodes set are excluded from sync.
	fn sync_with(&mut self, disconnect: bool, disconnected: Option<HashSet<usize>>) {
		self.start();
		while self.route_single(disconnect, disconnected.clone(), &|_| true) {
			// give protocol a chance to do its maintain procedures
			self.peers().iter().for_each(|peer| peer.sync_step());
		}
	}

	/// Deliver at most 1 pending message from every peer.
	fn sync_step(&mut self) {
		self.route_single(true, None, &|_| true);
	}

	/// Deliver pending messages until there are no more.
	fn sync(&mut self) {
		self.sync_with(true, None)
	}

	/// Deliver pending messages until there are no more. Do not disconnect nodes.
	fn sync_without_disconnects(&mut self) {
		self.sync_with(false, None)
	}

	/// Whether all peers have no pending outgoing messages.
	fn done(&self) -> bool {
		self.peers().iter().all(|p| p.is_done())
	}
}

pub struct TestNet {
	peers: Vec<Peer<(), DummySpecialization>>,
}

impl TestNetFactory for TestNet {
	type Specialization = DummySpecialization;
	type Verifier = PassThroughVerifier;
	type PeerData = ();

	/// Create new test network with peers and given config.
	fn from_config(_config: &ProtocolConfig) -> Self {
		TestNet {
			peers: Vec::new(),
		}
	}

	fn make_verifier(&self, _client: PeersClient, _config: &ProtocolConfig)
		-> Self::Verifier
	{
		PassThroughVerifier(false)
	}

	fn peer(&mut self, i: usize) -> &mut Peer<(), Self::Specialization> {
		&mut self.peers[i]
	}

	fn peers(&self) -> &Vec<Peer<(), Self::Specialization>> {
		&self.peers
	}

	fn mut_peers<F: FnOnce(&mut Vec<Peer<(), Self::Specialization>>)>(&mut self, closure: F) {
		closure(&mut self.peers);
	}
}

pub struct ForceFinalized(PeersClient);

impl JustificationImport<Block> for ForceFinalized {
	type Error = ConsensusError;

	fn import_justification(
		&mut self,
		hash: H256,
		_number: NumberFor<Block>,
		justification: Justification,
	) -> Result<(), Self::Error> {
		self.0.finalize_block(BlockId::Hash(hash), Some(justification), true)
			.map_err(|_| ConsensusError::InvalidJustification.into())
	}
}

pub struct JustificationTestNet(TestNet);

impl TestNetFactory for JustificationTestNet {
	type Specialization = DummySpecialization;
	type Verifier = PassThroughVerifier;
	type PeerData = ();

	fn from_config(config: &ProtocolConfig) -> Self {
		JustificationTestNet(TestNet::from_config(config))
	}

	fn make_verifier(&self, client: PeersClient, config: &ProtocolConfig) -> Self::Verifier {
		self.0.make_verifier(client, config)
	}

	fn peer(&mut self, i: usize) -> &mut Peer<Self::PeerData, Self::Specialization> {
		self.0.peer(i)
	}

	fn peers(&self) -> &Vec<Peer<Self::PeerData, Self::Specialization>> {
		self.0.peers()
	}

	fn mut_peers<F: FnOnce(&mut Vec<Peer<Self::PeerData, Self::Specialization>>)>(&mut self, closure: F) {
		self.0.mut_peers(closure)
	}

	fn make_block_import(&self, client: PeersClient)
		-> (
			BoxBlockImport<Block>,
			Option<BoxJustificationImport<Block>>,
			Option<BoxFinalityProofImport<Block>>,
			Option<BoxFinalityProofRequestBuilder<Block>>,
			Self::PeerData,
		)
	{
		(client.as_block_import(), Some(Box::new(ForceFinalized(client))), None, None, Default::default())
	}
}
