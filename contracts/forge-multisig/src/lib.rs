#![no_std]

//! # forge-multisig
//!
//! An N-of-M multisig treasury contract for Stellar/Soroban.
//!
//! ## Features
//! - N-of-M signature threshold for transaction approval
//! - Timelock delay before execution after approval
//! - Owners can propose, approve, reject, and execute transactions
//! - Native token support via Stellar token interface

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Vec,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owners,
    Threshold,
    TimelockDelay,
    Proposal(u64),
    NextProposalId,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A pending treasury transaction proposal.
#[contracttype]
#[derive(Clone)]
pub struct Proposal {
    /// Who proposed this transaction.
    pub proposer: Address,
    /// Destination address for the transfer.
    pub to: Address,
    /// Token address.
    pub token: Address,
    /// Amount to transfer.
    pub amount: i128,
    /// Addresses that have approved.
    pub approvals: Vec<Address>,
    /// Addresses that have rejected.
    pub rejections: Vec<Address>,
    /// Ledger timestamp when approval threshold was reached.
    pub approved_at: Option<u64>,
    /// Whether the proposal has been executed.
    pub executed: bool,
    /// Whether the proposal has been cancelled.
    pub cancelled: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum MultisigError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ProposalNotFound = 4,
    AlreadyVoted = 5,
    TimelockNotElapsed = 6,
    AlreadyExecuted = 7,
    AlreadyCancelled = 8,
    InsufficientApprovals = 9,
    InvalidThreshold = 10,
    InvalidAmount = 11,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MultisigContract;

#[contractimpl]
impl MultisigContract {
    /// Initialize the multisig treasury.
    ///
    /// # Parameters
    /// - `owners`: List of owner addresses.
    /// - `threshold`: Minimum approvals required (N-of-M).
    /// - `timelock_delay`: Seconds to wait after approval before execution.
    ///
    /// # Errors
    /// - `MultisigError::AlreadyInitialized` if already set up.
    /// - `MultisigError::InvalidThreshold` if threshold > owners count.
    pub fn initialize(
        env: Env,
        owners: Vec<Address>,
        threshold: u32,
        timelock_delay: u64,
    ) -> Result<(), MultisigError> {
        if env.storage().instance().has(&DataKey::Owners) {
            return Err(MultisigError::AlreadyInitialized);
        }
        if threshold == 0 || threshold > owners.len() {
            return Err(MultisigError::InvalidThreshold);
        }
        env.storage().instance().set(&DataKey::Owners, &owners);
        env.storage().instance().set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::TimelockDelay, &timelock_delay);
        Ok(())
    }

    /// Propose a token transfer from the treasury.
    ///
    /// # Returns
    /// The proposal ID.
    ///
    /// # Errors
    /// - `MultisigError::Unauthorized` if caller is not an owner.
    /// - `MultisigError::InvalidAmount` if amount is zero.
    pub fn propose(
        env: Env,
        proposer: Address,
        to: Address,
        token: Address,
        amount: i128,
    ) -> Result<u64, MultisigError> {
        proposer.require_auth();
        Self::require_owner(&env, &proposer)?;

        if amount <= 0 {
            return Err(MultisigError::InvalidAmount);
        }

        let proposal_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(0u64);

        let mut approvals = Vec::new(&env);
        approvals.push_back(proposer.clone());

        let proposal = Proposal {
            proposer,
            to,
            token,
            amount,
            approvals,
            rejections: Vec::new(&env),
            approved_at: None,
            executed: false,
            cancelled: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(proposal_id + 1));

        Ok(proposal_id)
    }

    /// Approve a proposal. If threshold is reached, starts the timelock.
    ///
    /// # Errors
    /// - `MultisigError::Unauthorized` if caller is not an owner.
    /// - `MultisigError::AlreadyVoted` if caller already voted.
    /// - `MultisigError::AlreadyExecuted` if already executed.
    pub fn approve(
        env: Env,
        owner: Address,
        proposal_id: u64,
    ) -> Result<(), MultisigError> {
        owner.require_auth();
        Self::require_owner(&env, &owner)?;

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(MultisigError::ProposalNotFound)?;

        if proposal.executed {
            return Err(MultisigError::AlreadyExecuted);
        }
        if proposal.cancelled {
            return Err(MultisigError::AlreadyCancelled);
        }
        if proposal.approvals.contains(&owner) || proposal.rejections.contains(&owner) {
            return Err(MultisigError::AlreadyVoted);
        }

        proposal.approvals.push_back(owner);

        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap();
        if proposal.approvals.len() >= threshold && proposal.approved_at.is_none() {
            proposal.approved_at = Some(env.ledger().timestamp());
        }

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        Ok(())
    }

    /// Reject a proposal.
    ///
    /// # Errors
    /// - `MultisigError::Unauthorized` if caller is not an owner.
    /// - `MultisigError::AlreadyVoted` if caller already voted.
    pub fn reject(
        env: Env,
        owner: Address,
        proposal_id: u64,
    ) -> Result<(), MultisigError> {
        owner.require_auth();
        Self::require_owner(&env, &owner)?;

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(MultisigError::ProposalNotFound)?;

        if proposal.executed {
            return Err(MultisigError::AlreadyExecuted);
        }
        if proposal.approvals.contains(&owner) || proposal.rejections.contains(&owner) {
            return Err(MultisigError::AlreadyVoted);
        }

        proposal.rejections.push_back(owner);
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        Ok(())
    }

    /// Execute an approved proposal after the timelock delay.
    ///
    /// # Errors
    /// - `MultisigError::InsufficientApprovals` if threshold not reached.
    /// - `MultisigError::TimelockNotElapsed` if timelock is still active.
    /// - `MultisigError::AlreadyExecuted` if already executed.
    pub fn execute(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), MultisigError> {
        executor.require_auth();
        Self::require_owner(&env, &executor)?;

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(MultisigError::ProposalNotFound)?;

        if proposal.executed {
            return Err(MultisigError::AlreadyExecuted);
        }
        if proposal.cancelled {
            return Err(MultisigError::AlreadyCancelled);
        }

        let approved_at = proposal.approved_at.ok_or(MultisigError::InsufficientApprovals)?;
        let delay: u64 = env.storage().instance().get(&DataKey::TimelockDelay).unwrap_or(0);

        if env.ledger().timestamp() < approved_at + delay {
            return Err(MultisigError::TimelockNotElapsed);
        }

        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        let token_client = token::Client::new(&env, &proposal.token);
        token_client.transfer(
            &env.current_contract_address(),
            &proposal.to,
            &proposal.amount,
        );

        Ok(())
    }

    /// Get a proposal by ID.
    pub fn get_proposal(env: Env, proposal_id: u64) -> Option<Proposal> {
        env.storage().persistent().get(&DataKey::Proposal(proposal_id))
    }

    /// Get list of owners.
    pub fn get_owners(env: Env) -> Vec<Address> {
        env.storage().instance().get(&DataKey::Owners).unwrap_or(Vec::new(&env))
    }

    /// Get the approval threshold.
    pub fn get_threshold(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Threshold).unwrap_or(0)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn require_owner(env: &Env, address: &Address) -> Result<(), MultisigError> {
        let owners: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Owners)
            .ok_or(MultisigError::NotInitialized)?;
        if owners.contains(address) {
            Ok(())
        } else {
            Err(MultisigError::Unauthorized)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, vec, Env};

    fn setup_2of3(env: &Env) -> (Address, Address, Address) {
        let o1 = Address::generate(env);
        let o2 = Address::generate(env);
        let o3 = Address::generate(env);
        let owners = vec![env, o1.clone(), o2.clone(), o3.clone()];
        MultisigContract::initialize(env.clone(), owners, 2, 3600).unwrap();
        (o1, o2, o3)
    }

    #[test]
    fn test_invalid_threshold() {
        let env = Env::default();
        env.mock_all_auths();
        env.register(MultisigContract, ());
        let o1 = Address::generate(&env);
        let owners = vec![&env, o1.clone()];
        let result = MultisigContract::initialize(env, owners, 5, 0);
        assert_eq!(result, Err(MultisigError::InvalidThreshold));
    }

    #[test]
    fn test_propose_and_approve_reaches_threshold() {
        let env = Env::default();
        env.mock_all_auths();
        env.register(MultisigContract, ());
        let (o1, o2, _) = setup_2of3(&env);
        let token = Address::generate(&env);
        let to = Address::generate(&env);

        let pid = MultisigContract::propose(env.clone(), o1, to, token, 500).unwrap();
        MultisigContract::approve(env.clone(), o2, pid).unwrap();

        let proposal = MultisigContract::get_proposal(env, pid).unwrap();
        assert!(proposal.approved_at.is_some());
    }

    #[test]
    fn test_double_vote_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.register(MultisigContract, ());
        let (o1, o2, _) = setup_2of3(&env);
        let token = Address::generate(&env);
        let to = Address::generate(&env);

        let pid = MultisigContract::propose(env.clone(), o1.clone(), to, token, 500).unwrap();
        let result = MultisigContract::approve(env, o1, pid);
        assert_eq!(result, Err(MultisigError::AlreadyVoted));
    }

    #[test]
    fn test_timelock_not_elapsed() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        env.register(MultisigContract, ());
        let (o1, o2, o3) = setup_2of3(&env);
        let token = Address::generate(&env);
        let to = Address::generate(&env);

        let pid = MultisigContract::propose(env.clone(), o1, to, token, 500).unwrap();
        MultisigContract::approve(env.clone(), o2, pid).unwrap();

        let result = MultisigContract::execute(env, o3, pid);
        assert_eq!(result, Err(MultisigError::TimelockNotElapsed));
    }

    #[test]
    fn test_execute_after_timelock() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        env.register(MultisigContract, ());
        let (o1, o2, o3) = setup_2of3(&env);
        let token = Address::generate(&env);
        let to = Address::generate(&env);

        let pid = MultisigContract::propose(env.clone(), o1, to, token, 500).unwrap();
        MultisigContract::approve(env.clone(), o2, pid).unwrap();

        env.ledger().with_mut(|l| l.timestamp = 7200);
        MultisigContract::execute(env.clone(), o3, pid).unwrap_or(());

        let proposal = MultisigContract::get_proposal(env, pid).unwrap();
        assert!(proposal.executed);
    }
}
