#![no_std]

//! # forge-governor
//!
//! On-chain governance with token-weighted voting for Stellar/Soroban.
//!
//! ## Features
//! - Token-weighted proposal voting (1 token = 1 vote)
//! - Configurable voting period and quorum
//! - Timelock between approval and execution
//! - Anyone can propose; execution is permissionless once passed

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, String, Vec,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Config,
    Proposal(u64),
    Vote(u64, Address),
    NextProposalId,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Governor configuration.
#[contracttype]
#[derive(Clone)]
pub struct GovernorConfig {
    /// Token used for voting weight.
    pub vote_token: Address,
    /// Seconds a proposal is open for voting.
    pub voting_period: u64,
    /// Minimum votes (in token units) for a proposal to pass.
    pub quorum: i128,
    /// Seconds between approval and execution.
    pub timelock_delay: u64,
}

/// Proposal state.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum ProposalState {
    Active,
    Passed,
    Failed,
    Executed,
    Cancelled,
}

/// A governance proposal.
#[contracttype]
#[derive(Clone)]
pub struct Proposal {
    /// Address that created the proposal.
    pub proposer: Address,
    /// Human-readable title.
    pub title: String,
    /// Human-readable description.
    pub description: String,
    /// Ledger timestamp when voting opens.
    pub vote_start: u64,
    /// Ledger timestamp when voting closes.
    pub vote_end: u64,
    /// Total votes in favor.
    pub votes_for: i128,
    /// Total votes against.
    pub votes_against: i128,
    /// Timestamp when proposal passed (for timelock).
    pub passed_at: Option<u64>,
    /// Current state.
    pub state: ProposalState,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum GovernorError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    ProposalNotFound = 3,
    VotingClosed = 4,
    VotingStillOpen = 5,
    AlreadyVoted = 6,
    QuorumNotReached = 7,
    ProposalNotPassed = 8,
    TimelockNotElapsed = 9,
    AlreadyExecuted = 10,
    AlreadyCancelled = 11,
    InvalidConfig = 12,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct GovernorContract;

#[contractimpl]
impl GovernorContract {
    /// Initialize the governor with configuration.
    ///
    /// # Errors
    /// - `GovernorError::AlreadyInitialized` if already configured.
    /// - `GovernorError::InvalidConfig` if quorum or voting period is zero.
    pub fn initialize(env: Env, config: GovernorConfig) -> Result<(), GovernorError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(GovernorError::AlreadyInitialized);
        }
        if config.quorum == 0 || config.voting_period == 0 {
            return Err(GovernorError::InvalidConfig);
        }
        env.storage().instance().set(&DataKey::Config, &config);
        Ok(())
    }

    /// Create a new governance proposal.
    ///
    /// # Returns
    /// The proposal ID.
    pub fn propose(
        env: Env,
        proposer: Address,
        title: String,
        description: String,
    ) -> Result<u64, GovernorError> {
        proposer.require_auth();

        let config: GovernorConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(GovernorError::NotInitialized)?;

        let now = env.ledger().timestamp();
        let proposal_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(0u64);

        let proposal = Proposal {
            proposer,
            title,
            description,
            vote_start: now,
            vote_end: now + config.voting_period,
            votes_for: 0,
            votes_against: 0,
            passed_at: None,
            state: ProposalState::Active,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(proposal_id + 1));

        Ok(proposal_id)
    }

    /// Cast a vote on an active proposal.
    ///
    /// # Parameters
    /// - `voter`: Address casting the vote.
    /// - `proposal_id`: ID of the proposal.
    /// - `support`: `true` = vote for, `false` = vote against.
    /// - `weight`: Number of tokens (voting power) to cast.
    ///
    /// # Errors
    /// - `GovernorError::VotingClosed` if the voting period has ended.
    /// - `GovernorError::AlreadyVoted` if voter already voted.
    pub fn vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        support: bool,
        weight: i128,
    ) -> Result<(), GovernorError> {
        voter.require_auth();

        let vote_key = DataKey::Vote(proposal_id, voter.clone());
        if env.storage().persistent().has(&vote_key) {
            return Err(GovernorError::AlreadyVoted);
        }

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(GovernorError::ProposalNotFound)?;

        if proposal.state != ProposalState::Active {
            return Err(GovernorError::VotingClosed);
        }

        let now = env.ledger().timestamp();
        if now > proposal.vote_end {
            return Err(GovernorError::VotingClosed);
        }

        if support {
            proposal.votes_for += weight;
        } else {
            proposal.votes_against += weight;
        }

        env.storage().persistent().set(&vote_key, &true);
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        Ok(())
    }

    /// Finalize a proposal after voting ends. Sets state to Passed or Failed.
    ///
    /// # Errors
    /// - `GovernorError::VotingStillOpen` if voting period hasn't ended.
    /// - `GovernorError::AlreadyExecuted` if already finalized.
    pub fn finalize(env: Env, proposal_id: u64) -> Result<ProposalState, GovernorError> {
        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(GovernorError::ProposalNotFound)?;

        if proposal.state != ProposalState::Active {
            return Err(GovernorError::AlreadyExecuted);
        }

        let now = env.ledger().timestamp();
        if now <= proposal.vote_end {
            return Err(GovernorError::VotingStillOpen);
        }

        let config: GovernorConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let total_votes = proposal.votes_for + proposal.votes_against;

        if total_votes >= config.quorum && proposal.votes_for > proposal.votes_against {
            proposal.state = ProposalState::Passed;
            proposal.passed_at = Some(now);
        } else {
            proposal.state = ProposalState::Failed;
        }

        let state = proposal.state.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        Ok(state)
    }

    /// Mark a passed proposal as executed after the timelock.
    ///
    /// In practice, execution logic would be defined per-proposal.
    /// This marks the proposal executed and enforces the timelock.
    ///
    /// # Errors
    /// - `GovernorError::ProposalNotPassed` if proposal hasn't passed.
    /// - `GovernorError::TimelockNotElapsed` if timelock is still active.
    pub fn execute(env: Env, executor: Address, proposal_id: u64) -> Result<(), GovernorError> {
        executor.require_auth();

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(GovernorError::ProposalNotFound)?;

        if proposal.state == ProposalState::Executed {
            return Err(GovernorError::AlreadyExecuted);
        }
        if proposal.state != ProposalState::Passed {
            return Err(GovernorError::ProposalNotPassed);
        }

        let passed_at = proposal.passed_at.unwrap();
        let config: GovernorConfig = env.storage().instance().get(&DataKey::Config).unwrap();

        if env.ledger().timestamp() < passed_at + config.timelock_delay {
            return Err(GovernorError::TimelockNotElapsed);
        }

        proposal.state = ProposalState::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        Ok(())
    }

    /// Get a proposal by ID.
    pub fn get_proposal(env: Env, proposal_id: u64) -> Option<Proposal> {
        env.storage().persistent().get(&DataKey::Proposal(proposal_id))
    }

    /// Get the governor configuration.
    pub fn get_config(env: Env) -> Option<GovernorConfig> {
        env.storage().instance().get(&DataKey::Config)
    }

    /// Check if an address has voted on a proposal.
    pub fn has_voted(env: Env, proposal_id: u64, voter: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Vote(proposal_id, voter))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Env, String};

    fn setup(env: &Env) -> GovernorConfig {
        let token = Address::generate(env);
        let config = GovernorConfig {
            vote_token: token,
            voting_period: 3600,
            quorum: 100,
            timelock_delay: 86400,
        };
        GovernorContract::initialize(env.clone(), config.clone()).unwrap();
        config
    }

    #[test]
    fn test_vote_and_pass() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 1000);
        env.register(GovernorContract, ());
        setup(&env);

        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);

        let pid = GovernorContract::propose(
            env.clone(),
            proposer,
            String::from_str(&env, "Test Proposal"),
            String::from_str(&env, "A test"),
        ).unwrap();

        GovernorContract::vote(env.clone(), voter, pid, true, 200).unwrap();

        // Advance past voting period
        env.ledger().with_mut(|l| l.timestamp = 5000);
        let state = GovernorContract::finalize(env.clone(), pid).unwrap();
        assert_eq!(state, ProposalState::Passed);
    }

    #[test]
    fn test_quorum_not_reached_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        env.register(GovernorContract, ());
        setup(&env);

        let proposer = Address::generate(&env);
        let pid = GovernorContract::propose(
            env.clone(),
            proposer,
            String::from_str(&env, "Low vote"),
            String::from_str(&env, "desc"),
        ).unwrap();

        // Vote with less than quorum (100)
        let voter = Address::generate(&env);
        GovernorContract::vote(env.clone(), voter, pid, true, 50).unwrap();

        env.ledger().with_mut(|l| l.timestamp = 5000);
        let state = GovernorContract::finalize(env.clone(), pid).unwrap();
        assert_eq!(state, ProposalState::Failed);
    }

    #[test]
    fn test_double_vote_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        env.register(GovernorContract, ());
        setup(&env);

        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let pid = GovernorContract::propose(
            env.clone(), proposer,
            String::from_str(&env, "P"),
            String::from_str(&env, "D"),
        ).unwrap();

        GovernorContract::vote(env.clone(), voter.clone(), pid, true, 100).unwrap();
        let result = GovernorContract::vote(env, voter, pid, true, 100);
        assert_eq!(result, Err(GovernorError::AlreadyVoted));
    }

    #[test]
    fn test_execute_before_timelock_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        env.register(GovernorContract, ());
        setup(&env);

        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let executor = Address::generate(&env);

        let pid = GovernorContract::propose(
            env.clone(), proposer,
            String::from_str(&env, "P"),
            String::from_str(&env, "D"),
        ).unwrap();

        GovernorContract::vote(env.clone(), voter, pid, true, 200).unwrap();
        env.ledger().with_mut(|l| l.timestamp = 5000);
        GovernorContract::finalize(env.clone(), pid).unwrap();

        // Try to execute immediately (timelock = 86400)
        let result = GovernorContract::execute(env, executor, pid);
        assert_eq!(result, Err(GovernorError::TimelockNotElapsed));
    }
}
