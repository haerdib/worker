/*
	Copyright 2021 Integritee AG and Supercomputing Systems AG

	Licensed under the Apache License, Version 2.0 (the "License");
	you may not use this file except in compliance with the License.
	You may obtain a copy of the License at

		http://www.apache.org/licenses/LICENSE-2.0

	Unless required by applicable law or agreed to in writing, software
	distributed under the License is distributed on an "AS IS" BASIS,
	WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
	See the License for the specific language governing permissions and
	limitations under the License.

*/

use crate::{
	error::{Error, Result},
	traits::{StfEnclaveSigning, StfShardVaultQuery},
	H256,
};
use codec::{Decode, Encode};
use core::{fmt::Debug, marker::PhantomData};
use itp_ocall_api::EnclaveAttestationOCallApi;
use itp_sgx_crypto::{ed25519_derivation::DeriveEd25519, key_repository::AccessKey};
use itp_sgx_externalities::SgxExternalitiesTrait;
use itp_stf_interface::{system_pallet::SystemPalletAccountInterface, ShardVaultQuery};
use itp_stf_primitives::{
	traits::TrustedCallSigning,
	types::{AccountId, KeyPair},
};
use itp_stf_state_observer::traits::ObserveState;
use itp_top_pool_author::traits::AuthorApi;
use itp_types::{Index, ShardIdentifier};
use sp_core::{ed25519::Pair as Ed25519Pair, Pair};
use std::{boxed::Box, sync::Arc};

pub struct StfEnclaveSigner<
	OCallApi,
	StateObserver,
	ShieldingKeyRepository,
	Stf,
	TopPoolAuthor,
	TCS,
	G,
> {
	state_observer: Arc<StateObserver>,
	ocall_api: Arc<OCallApi>,
	shielding_key_repo: Arc<ShieldingKeyRepository>,
	top_pool_author: Arc<TopPoolAuthor>,
	_phantom: PhantomData<(Stf, TCS, G)>,
}

impl<OCallApi, StateObserver, ShieldingKeyRepository, Stf, TopPoolAuthor, TCS, G>
	StfEnclaveSigner<OCallApi, StateObserver, ShieldingKeyRepository, Stf, TopPoolAuthor, TCS, G>
where
	OCallApi: EnclaveAttestationOCallApi,
	StateObserver: ObserveState,
	StateObserver::StateType: SgxExternalitiesTrait,
	ShieldingKeyRepository: AccessKey,
	<ShieldingKeyRepository as AccessKey>::KeyType: DeriveEd25519,
	Stf: SystemPalletAccountInterface<StateObserver::StateType, AccountId>
		+ ShardVaultQuery<StateObserver::StateType>,
	Stf::Index: Into<Index>,
	TopPoolAuthor: AuthorApi<H256, H256, TCS, G> + Send + Sync + 'static,
	TCS: PartialEq + Encode + Decode + Debug + Send + Sync,
	G: PartialEq + Encode + Decode + Debug + Send + Sync,
{
	pub fn new(
		state_observer: Arc<StateObserver>,
		ocall_api: Arc<OCallApi>,
		shielding_key_repo: Arc<ShieldingKeyRepository>,
		top_pool_author: Arc<TopPoolAuthor>,
	) -> Self {
		Self {
			state_observer,
			ocall_api,
			shielding_key_repo,
			top_pool_author,
			_phantom: Default::default(),
		}
	}

	fn get_enclave_account_nonce(&self, shard: &ShardIdentifier) -> Result<Stf::Index> {
		let enclave_account = self.get_enclave_account()?;
		let nonce = self
			.state_observer
			.observe_state(shard, move |state| Stf::get_account_nonce(state, &enclave_account))?;

		Ok(nonce)
	}

	fn get_enclave_call_signing_key(&self) -> Result<Ed25519Pair> {
		let shielding_key = self.shielding_key_repo.retrieve_key()?;
		shielding_key.derive_ed25519().map_err(|e| e.into())
	}
}

impl<OCallApi, StateObserver, ShieldingKeyRepository, Stf, TopPoolAuthor, TCS, G>
	StfEnclaveSigning<TCS>
	for StfEnclaveSigner<OCallApi, StateObserver, ShieldingKeyRepository, Stf, TopPoolAuthor, TCS, G>
where
	OCallApi: EnclaveAttestationOCallApi,
	StateObserver: ObserveState,
	StateObserver::StateType: SgxExternalitiesTrait,
	ShieldingKeyRepository: AccessKey,
	<ShieldingKeyRepository as AccessKey>::KeyType: DeriveEd25519,
	Stf: SystemPalletAccountInterface<StateObserver::StateType, AccountId>
		+ ShardVaultQuery<StateObserver::StateType>,
	Stf::Index: Into<Index>,
	TopPoolAuthor: AuthorApi<H256, H256, TCS, G> + Send + Sync + 'static,
	TCS: PartialEq + Encode + Decode + Debug + Send + Sync,
	G: PartialEq + Encode + Decode + Debug + Send + Sync,
{
	fn get_enclave_account(&self) -> Result<AccountId> {
		let enclave_call_signing_key = self.get_enclave_call_signing_key()?;
		Ok(enclave_call_signing_key.public().into())
	}

	fn sign_call_with_self<TC: Encode + Debug + TrustedCallSigning<TCS>>(
		&self,
		trusted_call: &TC,
		shard: &ShardIdentifier,
	) -> Result<TCS> {
		let mr_enclave = self.ocall_api.get_mrenclave_of_self()?;
		let enclave_account = self.get_enclave_account()?;
		let enclave_call_signing_key = self.get_enclave_call_signing_key()?;

		let current_nonce = self.get_enclave_account_nonce(shard)?;
		let pending_tx_count = self
			.top_pool_author
			.get_pending_trusted_calls_for(*shard, &enclave_account)
			.len();
		let pending_tx_count =
			Index::try_from(pending_tx_count).map_err(|e| Error::Other(e.into()))?;
		let adjusted_nonce: Index = current_nonce.into() + pending_tx_count;

		Ok(trusted_call.sign(
			&KeyPair::Ed25519(Box::new(enclave_call_signing_key)),
			adjusted_nonce,
			&mr_enclave.m,
			shard,
		))
	}
}

impl<OCallApi, StateObserver, ShieldingKeyRepository, Stf, TopPoolAuthor, TCS, G> StfShardVaultQuery
	for StfEnclaveSigner<OCallApi, StateObserver, ShieldingKeyRepository, Stf, TopPoolAuthor, TCS, G>
where
	OCallApi: EnclaveAttestationOCallApi,
	StateObserver: ObserveState,
	StateObserver::StateType: SgxExternalitiesTrait,
	ShieldingKeyRepository: AccessKey,
	<ShieldingKeyRepository as AccessKey>::KeyType: DeriveEd25519,
	Stf: SystemPalletAccountInterface<StateObserver::StateType, AccountId>
		+ ShardVaultQuery<StateObserver::StateType>,
	Stf::Index: Into<Index>,
	TopPoolAuthor: AuthorApi<H256, H256, TCS, G> + Send + Sync + 'static,
	TCS: PartialEq + Encode + Decode + Debug + Send + Sync,
	G: PartialEq + Encode + Decode + Debug + Send + Sync,
{
	fn get_shard_vault(&self, shard: &ShardIdentifier) -> Result<AccountId> {
		let vault = self.state_observer.observe_state(shard, move |state| Stf::get_vault(state))?;

		vault.ok_or_else(|| Error::Other("shard vault undefined".into()))
	}
}
