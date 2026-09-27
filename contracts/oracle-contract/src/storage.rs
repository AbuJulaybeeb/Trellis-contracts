use soroban_sdk::{symbol_short, Address, Env, Map, Symbol, Vec};
use crate::types::{FeedLatest, PriceSubmission, SubmitterInfo};
use shared::storage::{persistent_get, persistent_set};

// Storage key symbols (all <= 9 chars for symbol_short!)
const KEY_SUBMITTERS: Symbol = symbol_short!("submits");
const KEY_FEED_LATEST: Symbol = symbol_short!("feed_lat");
const KEY_FEED_HISTORY: Symbol = symbol_short!("feed_his");
const KEY_NEXT_SUBMISSION_ID: Symbol = symbol_short!("sub_id");
const KEY_NONCE: Symbol = symbol_short!("nonce");
const KEY_STALENESS_WINDOW: Symbol = symbol_short!("stale_wd");

/// Get or initialize the submitters map.
pub fn get_submitters(env: &Env) -> Map<Address, SubmitterInfo> {
    env.storage()
        .instance()
        .get(&KEY_SUBMITTERS)
        .unwrap_or_else(|| Map::new(env))
}

/// Set the submitters map.
pub fn set_submitters(env: &Env, submitters: &Map<Address, SubmitterInfo>) {
    env.storage().instance().set(&KEY_SUBMITTERS, submitters);
}

/// Check if a submitter is registered and active.
pub fn is_submitter_active(env: &Env, submitter: &Address) -> bool {
    get_submitters(env)
        .get(submitter.clone())
        .map(|info| info.active)
        .unwrap_or(false)
}

/// Register a new submitter.
pub fn register_submitter(
    env: &Env,
    submitter: Address,
    registered_at: u64,
) -> Result<(), crate::errors::OracleError> {
    let mut submitters = get_submitters(env);
    if submitters.get(submitter.clone()).is_some() {
        return Err(crate::errors::OracleError::SubmitterAlreadyRegistered);
    }
    let info = SubmitterInfo {
        address: submitter.clone(),
        active: true,
        registered_at,
    };
    submitters.set(submitter, info);
    set_submitters(env, &submitters);
    Ok(())
}

/// Deactivate a submitter.
pub fn deactivate_submitter(env: &Env, submitter: &Address) {
    let mut submitters = get_submitters(env);
    if let Some(mut info) = submitters.get(submitter.clone()) {
        info.active = false;
        submitters.set(submitter.clone(), info);
        set_submitters(env, &submitters);
    }
}

/// Get the latest price for a feed.
pub fn get_feed_latest(env: &Env, feed_id: &Symbol) -> Option<FeedLatest> {
    let feeds: Map<Symbol, FeedLatest> = env
        .storage()
        .instance()
        .get(&KEY_FEED_LATEST)
        .unwrap_or_else(|| Map::new(env));
    feeds.get(feed_id.clone())
}

/// Set or update the latest price for a feed.
pub fn set_feed_latest(env: &Env, feed_id: &Symbol, latest: &FeedLatest) {
    let mut feeds: Map<Symbol, FeedLatest> = env
        .storage()
        .instance()
        .get(&KEY_FEED_LATEST)
        .unwrap_or_else(|| Map::new(env));
    feeds.set(feed_id.clone(), latest.clone());
    env.storage().instance().set(&KEY_FEED_LATEST, &feeds);
}

/// Get submission history for a feed (limited to most recent N submissions).
pub fn get_feed_history(env: &Env, feed_id: &Symbol, limit: u32) -> Vec<PriceSubmission> {
    let history: Map<Symbol, Vec<PriceSubmission>> = persistent_get(env, &KEY_FEED_HISTORY)
        .unwrap_or_else(|| Map::new(env));
    
    if let Some(submissions) = history.get(feed_id.clone()) {
        let len = submissions.len() as u32;
        let start = if len > limit {
            (len - limit) as u32
        } else {
            0
        };
        let mut result = Vec::new(env);
        let mut i = start;
        while i < submissions.len() {
            if let Some(sub) = submissions.get(i) {
                result.push_back(sub);
            }
            i += 1;
        }
        result
    } else {
        Vec::new(env)
    }
}

/// Append a submission to the history for a feed.
pub fn append_submission(env: &Env, feed_id: &Symbol, submission: &PriceSubmission) {
    let mut history: Map<Symbol, Vec<PriceSubmission>> = persistent_get(env, &KEY_FEED_HISTORY)
        .unwrap_or_else(|| Map::new(env));
    
    let mut submissions = history.get(feed_id.clone()).unwrap_or_else(|| Vec::new(env));
    submissions.push_back(submission.clone());
    history.set(feed_id.clone(), submissions);
    persistent_set(env, &KEY_FEED_HISTORY, &history);
}

/// Get the next submission ID and increment the counter.
pub fn next_submission_id(env: &Env) -> Result<u64, crate::errors::OracleError> {
    let current: u64 = env
        .storage()
        .instance()
        .get(&KEY_NEXT_SUBMISSION_ID)
        .unwrap_or(0);
    let next = current
        .checked_add(1)
        .ok_or(crate::errors::OracleError::InternalError)?;
    env.storage().instance().set(&KEY_NEXT_SUBMISSION_ID, &next);
    Ok(next)
}

/// Get the nonce for a submitter and feed (for replay protection).
pub fn get_nonce(env: &Env, submitter: &Address, feed_id: &Symbol) -> u64 {
    let nonces: Map<(Address, Symbol), u64> = env
        .storage()
        .instance()
        .get(&KEY_NONCE)
        .unwrap_or_else(|| Map::new(env));
    nonces
        .get((submitter.clone(), feed_id.clone()))
        .unwrap_or(0)
}

/// Set the nonce for a submitter and feed.
pub fn set_nonce(env: &Env, submitter: &Address, feed_id: &Symbol, nonce: u64) {
    let mut nonces: Map<(Address, Symbol), u64> = env
        .storage()
        .instance()
        .get(&KEY_NONCE)
        .unwrap_or_else(|| Map::new(env));
    nonces.set((submitter.clone(), feed_id.clone()), nonce);
    env.storage().instance().set(&KEY_NONCE, &nonces);
}

/// Get the staleness window (in seconds) for feed submissions.
pub fn get_staleness_window(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&KEY_STALENESS_WINDOW)
        .unwrap_or(3600) // Default: 1 hour
}

/// Set the staleness window.
pub fn set_staleness_window(env: &Env, seconds: u64) {
    env.storage()
        .instance()
        .set(&KEY_STALENESS_WINDOW, &seconds);
}
