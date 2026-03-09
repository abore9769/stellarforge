#![no_std]

//! # forge-stream
//!
//! Real-time token streaming — pay-per-second token transfers on Soroban.
//!
//! ## Overview
//! - Sender creates a stream with a rate (tokens per second) and duration
//! - Recipient can withdraw accrued tokens at any time
//! - Sender can cancel and reclaim unstreamed tokens
//! - Multiple streams can run in parallel (keyed by stream_id)

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Symbol,
};

#[contracttype]
pub enum DataKey {
    Stream(u64),
    NextId,
}

#[contracttype]
#[derive(Clone)]
pub struct Stream {
    /// Unique stream ID
    pub id: u64,
    /// Token being streamed
    pub token: Address,
    /// Sender funding the stream
    pub sender: Address,
    /// Recipient receiving tokens
    pub recipient: Address,
    /// Tokens per second
    pub rate_per_second: i128,
    /// Stream start timestamp
    pub start_time: u64,
    /// Stream end timestamp
    pub end_time: u64,
    /// Total tokens already withdrawn
    pub withdrawn: i128,
    /// Whether the stream has been cancelled
    pub cancelled: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct StreamStatus {
    pub id: u64,
    pub streamed: i128,
    pub withdrawn: i128,
    pub withdrawable: i128,
    pub remaining: i128,
    pub is_active: bool,
    pub is_finished: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum StreamError {
    StreamNotFound = 1,
    Unauthorized = 2,
    NothingToWithdraw = 3,
    AlreadyCancelled = 4,
    InvalidConfig = 5,
    StreamFinished = 6,
}

#[contract]
pub struct ForgeStream;

#[contractimpl]
impl ForgeStream {
    /// Create a new token stream.
    ///
    /// # Parameters
    /// - `token`: Token contract address
    /// - `recipient`: Address that receives streamed tokens
    /// - `rate_per_second`: Tokens unlocked per second
    /// - `duration_seconds`: How long the stream runs
    ///
    /// Returns the new stream ID.
    pub fn create_stream(
        env: Env,
        sender: Address,
        token: Address,
        recipient: Address,
        rate_per_second: i128,
        duration_seconds: u64,
    ) -> Result<u64, StreamError> {
        if rate_per_second <= 0 || duration_seconds == 0 {
            return Err(StreamError::InvalidConfig);
        }

        sender.require_auth();

        let stream_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextId)
            .unwrap_or(0_u64);

        let now = env.ledger().timestamp();
        let total = rate_per_second * duration_seconds as i128;

        // Pull total tokens from sender into contract
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &env.current_contract_address(), &total);

        let stream = Stream {
            id: stream_id,
            token,
            sender,
            recipient,
            rate_per_second,
            start_time: now,
            end_time: now + duration_seconds,
            withdrawn: 0,
            cancelled: false,
        };

        env.storage()
            .instance()
            .set(&DataKey::Stream(stream_id), &stream);
        env.storage()
            .instance()
            .set(&DataKey::NextId, &(stream_id + 1));

        env.events().publish(
            (Symbol::new(&env, "stream_created"),),
            (stream_id, &stream.recipient, rate_per_second, duration_seconds),
        );

        Ok(stream_id)
    }

    /// Withdraw all accrued tokens from a stream.
    /// Only callable by the stream recipient.
    pub fn withdraw(env: Env, stream_id: u64) -> Result<i128, StreamError> {
        let mut stream: Stream = env
            .storage()
            .instance()
            .get(&DataKey::Stream(stream_id))
            .ok_or(StreamError::StreamNotFound)?;

        if stream.cancelled {
            return Err(StreamError::AlreadyCancelled);
        }

        stream.recipient.require_auth();

        let now = env.ledger().timestamp();
        let streamed = Self::compute_streamed(&stream, now);
        let withdrawable = streamed - stream.withdrawn;

        if withdrawable <= 0 {
            return Err(StreamError::NothingToWithdraw);
        }

        stream.withdrawn += withdrawable;
        env.storage()
            .instance()
            .set(&DataKey::Stream(stream_id), &stream);

        let token_client = token::Client::new(&env, &stream.token);
        token_client.transfer(
            &env.current_contract_address(),
            &stream.recipient,
            &withdrawable,
        );

        env.events().publish(
            (Symbol::new(&env, "withdrawn"),),
            (stream_id, &stream.recipient, withdrawable),
        );

        Ok(withdrawable)
    }

    /// Cancel a stream. Unstreamed tokens are returned to sender.
    /// Only callable by the stream sender.
    pub fn cancel_stream(env: Env, stream_id: u64) -> Result<(), StreamError> {
        let mut stream: Stream = env
            .storage()
            .instance()
            .get(&DataKey::Stream(stream_id))
            .ok_or(StreamError::StreamNotFound)?;

        if stream.cancelled {
            return Err(StreamError::AlreadyCancelled);
        }

        stream.sender.require_auth();

        let now = env.ledger().timestamp();
        let streamed = Self::compute_streamed(&stream, now);
        let withdrawable = (streamed - stream.withdrawn).max(0);
        let total = stream.rate_per_second * (stream.end_time - stream.start_time) as i128;
        let returnable = total - streamed;

        stream.cancelled = true;
        env.storage()
            .instance()
            .set(&DataKey::Stream(stream_id), &stream);

        let token_client = token::Client::new(&env, &stream.token);

        // Pay out accrued amount to recipient
        if withdrawable > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &stream.recipient,
                &withdrawable,
            );
        }

        // Return unstreamed amount to sender
        if returnable > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &stream.sender,
                &returnable,
            );
        }

        env.events().publish(
            (Symbol::new(&env, "stream_cancelled"),),
            (stream_id, withdrawable, returnable),
        );

        Ok(())
    }

    /// Get the current status of a stream.
    pub fn get_stream_status(env: Env, stream_id: u64) -> Result<StreamStatus, StreamError> {
        let stream: Stream = env
            .storage()
            .instance()
            .get(&DataKey::Stream(stream_id))
            .ok_or(StreamError::StreamNotFound)?;

        let now = env.ledger().timestamp();
        let streamed = Self::compute_streamed(&stream, now);
        let withdrawable = (streamed - stream.withdrawn).max(0);
        let total = stream.rate_per_second * (stream.end_time - stream.start_time) as i128;
        let remaining = (total - streamed).max(0);
        let is_active = !stream.cancelled && now < stream.end_time;
        let is_finished = now >= stream.end_time;

        Ok(StreamStatus {
            id: stream.id,
            streamed,
            withdrawn: stream.withdrawn,
            withdrawable,
            remaining,
            is_active,
            is_finished,
        })
    }

    /// Get full stream data.
    pub fn get_stream(env: Env, stream_id: u64) -> Result<Stream, StreamError> {
        env.storage()
            .instance()
            .get(&DataKey::Stream(stream_id))
            .ok_or(StreamError::StreamNotFound)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn compute_streamed(stream: &Stream, now: u64) -> i128 {
        if stream.cancelled {
            return stream.withdrawn;
        }
        let effective_time = now.min(stream.end_time);
        let elapsed = effective_time.saturating_sub(stream.start_time);
        stream.rate_per_second * elapsed as i128
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Env};

    #[test]
    fn test_create_stream_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        let result = client.try_create_stream(&sender, &token, &recipient, &100, &1000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_invalid_stream_config() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        let result = client.try_create_stream(&sender, &token, &recipient, &0, &1000);
        assert_eq!(result, Err(Ok(StreamError::InvalidConfig)));
    }

    #[test]
    fn test_stream_not_found() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let result = client.try_withdraw(&999);
        assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
    }

    #[test]
    fn test_withdraw_nothing_to_withdraw() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        let stream_id = client.create_stream(&sender, &token, &recipient, &100, &1000);
        // No time has passed — nothing to withdraw
        let result = client.try_withdraw(&stream_id);
        assert_eq!(result, Err(Ok(StreamError::NothingToWithdraw)));
    }

    #[test]
    fn test_stream_status_active() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        let stream_id = client.create_stream(&sender, &token, &recipient, &100, &1000);
        env.ledger().with_mut(|l| l.timestamp += 100);

        let status = client.get_stream_status(&stream_id).unwrap();
        assert!(status.is_active);
        assert_eq!(status.streamed, 10_000); // 100 * 100s
        assert_eq!(status.withdrawable, 10_000);
    }

    #[test]
    fn test_cancel_stream() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        let stream_id = client.create_stream(&sender, &token, &recipient, &100, &1000);
        let result = client.try_cancel_stream(&stream_id);
        assert!(result.is_ok());

        let result2 = client.try_cancel_stream(&stream_id);
        assert_eq!(result2, Err(Ok(StreamError::AlreadyCancelled)));
    }

    #[test]
    fn test_stream_finished_after_duration() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ForgeStream, ());
        let client = ForgeStreamClient::new(&env, &contract_id);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        let stream_id = client.create_stream(&sender, &token, &recipient, &100, &1000);
        env.ledger().with_mut(|l| l.timestamp += 2000);

        let status = client.get_stream_status(&stream_id).unwrap();
        assert!(status.is_finished);
        assert!(!status.is_active);
        assert_eq!(status.streamed, 100_000); // 100 * 1000s = full amount
    }
}
