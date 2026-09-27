use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Val};
use crate::storage::{persistent_get, persistent_set};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRecord {
    pub id: u64,
    pub actor: Address,
    pub reason: Symbol,
    pub before_hash: BytesN<32>,
    pub after_hash: BytesN<32>,
    pub prev_history_hash: BytesN<32>,
    pub timestamp: u64,
}

#[contracttype]
pub enum HistoryKey {
    LatestHistoryHash(Val),
    HistoryEntry(Val, u64),
}

pub fn record_mutation(
    env: &Env,
    record_id: Val,
    actor: Address,
    reason: Symbol,
    before_hash: BytesN<32>,
    after_hash: BytesN<32>,
) -> BytesN<32> {
    let latest_key = HistoryKey::LatestHistoryHash(record_id.clone());
    let (prev_hash, id) = persistent_get::<_, (BytesN<32>, u64)>(env, &latest_key)
        .unwrap_or_else(|| (BytesN::from_array(env, &[0; 32]), 0));

    let record = HistoryRecord {
        id: id + 1,
        actor,
        reason,
        before_hash,
        after_hash,
        prev_history_hash: prev_hash,
        timestamp: env.ledger().timestamp(),
    };

    let mut b = Bytes::new(env);
    b.append(&record.id.clone().into_val(env));
    // simplistic hashing for proof of concept
    let new_hash = env.crypto().sha256(&b);

    persistent_set(env, &HistoryKey::HistoryEntry(record_id.clone(), record.id), &record);
    persistent_set(env, &latest_key, &(new_hash.clone(), record.id));
    new_hash
}
