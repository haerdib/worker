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
	error::Result,
	state_getter::GetState,
	traits::{StateUpdateProposer, StfEnclaveSigning},
	BatchExecutionResult, ExecutedOperation,
};
use codec::{Decode, Encode};
use core::fmt::Debug;
use itp_sgx_externalities::SgxExternalitiesTrait;
use itp_stf_primitives::{
	traits::TrustedCallSigning,
	types::{AccountId, KeyPair, ShardIdentifier, TrustedOperationOrHash},
};
use itp_types::H256;
use sp_core::Pair;
use sp_runtime::traits::Header as HeaderTrait;
#[cfg(feature = "std")]
use std::sync::RwLock;
use std::{boxed::Box, marker::PhantomData, ops::Deref, time::Duration, vec::Vec};

use crate::traits::StfShardVaultQuery;
use itp_stf_primitives::{
	traits::{GetterAuthorization, TrustedCallVerification},
	types::TrustedOperation,
};
#[cfg(feature = "sgx")]
use std::sync::SgxRwLock as RwLock;

/// Mock for the StfExecutor.
#[derive(Default)]
pub struct StfExecutorMock<State> {
	pub state: RwLock<State>,
}

impl<State: Clone> StfExecutorMock<State> {
	pub fn new(state: State) -> Self {
		Self { state: RwLock::new(state) }
	}

	pub fn get_state(&self) -> State {
		(*self.state.read().unwrap().deref()).clone()
	}
}

impl<State, TCS, G> StateUpdateProposer<TCS, G> for StfExecutorMock<State>
where
	State: SgxExternalitiesTrait + Encode + Clone,
	TCS: PartialEq + Encode + Decode + Clone + Debug + Send + Sync + TrustedCallVerification,
	G: PartialEq + Encode + Decode + Clone + Debug + Send + Sync,
{
	type Externalities = State;

	fn propose_state_update<PH, F>(
		&self,
		trusted_calls: &[TrustedOperation<TCS, G>],
		_header: &PH,
		_shard: &ShardIdentifier,
		_max_exec_duration: Duration,
		prepare_state_function: F,
	) -> Result<BatchExecutionResult<Self::Externalities, TCS, G>>
	where
		PH: HeaderTrait<Hash = H256>,
		F: FnOnce(Self::Externalities) -> Self::Externalities,
	{
		let mut lock = self.state.write().unwrap();

		let updated_state = prepare_state_function((*lock.deref()).clone());

		*lock = updated_state.clone();

		let executed_operations: Vec<ExecutedOperation<TCS, G>> = trusted_calls
			.iter()
			.map(|c| {
				let operation_hash = c.hash();
				let top_or_hash = TrustedOperationOrHash::<TCS, G>::from_top(c.clone());
				ExecutedOperation::success(operation_hash, top_or_hash, Vec::new())
			})
			.collect();

		Ok(BatchExecutionResult {
			executed_operations,
			state_hash_before_execution: H256::default(),
			state_after_execution: updated_state,
		})
	}
}

/// Enclave signer mock.
pub struct StfEnclaveSignerMock {
	mr_enclave: [u8; 32],
	signer: sp_core::ed25519::Pair,
}

impl StfEnclaveSignerMock {
	pub fn new(mr_enclave: [u8; 32]) -> Self {
		type Seed = [u8; 32];
		const TEST_SEED: Seed = *b"42345678901234567890123456789012";

		Self { mr_enclave, signer: sp_core::ed25519::Pair::from_seed(&TEST_SEED) }
	}
}

impl Default for StfEnclaveSignerMock {
	fn default() -> Self {
		Self::new([0u8; 32])
	}
}

impl<TCS: PartialEq + Encode + Debug> StfEnclaveSigning<TCS> for StfEnclaveSignerMock {
	fn get_enclave_account(&self) -> Result<AccountId> {
		Ok(self.signer.public().into())
	}

	fn sign_call_with_self<TC: Encode + Debug + TrustedCallSigning<TCS>>(
		&self,
		trusted_call: &TC,
		shard: &ShardIdentifier,
	) -> Result<TCS> {
		Ok(trusted_call.sign(&KeyPair::Ed25519(Box::new(self.signer)), 1, &self.mr_enclave, shard))
	}
}

impl StfShardVaultQuery for StfEnclaveSignerMock {
	fn get_shard_vault(&self, _shard: &ShardIdentifier) -> Result<AccountId> {
		Err(crate::error::Error::Other("shard vault undefined".into()))
	}
}

/// GetState mock
#[derive(Default)]
pub struct GetStateMock<StateType> {
	_phantom: PhantomData<StateType>,
}

impl<StateType, G> GetState<StateType, G> for GetStateMock<StateType>
where
	StateType: Encode,
	G: PartialEq + Decode + GetterAuthorization,
{
	fn get_state(_getter: G, state: &mut StateType) -> Result<Option<Vec<u8>>> {
		Ok(Some(state.encode()))
	}
}
