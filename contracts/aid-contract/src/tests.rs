#![cfg(test)]

extern crate std;

use super::*;
use shared::Error as SharedError;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Env,
};
#[allow(dead_code)]
fn setup_token<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract(admin.clone());
    let client = token::Client::new(env, &contract_address);
    let asset_client = token::StellarAssetClient::new(env, &contract_address);
    (contract_address, client, asset_client)
}

const MINT_AMOUNT: i128 = 1_000_000;

struct Fixture {
    env: Env,
    admin: Address,
    donor: Address,
    recipient: Address,
    token_addr: Address,
    contract_id: Address,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let donor = Address::generate(&env);
    let recipient = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract(admin.clone());
    let asset_client = token::StellarAssetClient::new(&env, &token_addr);
    asset_client.mint(&donor, &MINT_AMOUNT);

    let contract_id = env.register_contract(None, AidContract);
    let client = AidContractClient::new(&env, &contract_id);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &treasury, &token_addr, &3600);

    Fixture {
        env,
        admin,
        donor,
        recipient,
        token_addr,
        contract_id,
    }
}

/// Create `count` aids of 100 units each from `donor` to `recipient`,
/// returning the allocated IDs in creation order.
fn create_aids(
    env: &Env,
    client: &AidContractClient,
    donor: &Address,
    recipient: &Address,
    count: u32,
) -> std::vec::Vec<u64> {
    let expiry = env.ledger().sequence() + 10_000;
    let mut ids = std::vec::Vec::with_capacity(count as usize);
    for _ in 0..count {
        ids.push(client.create_aid(donor, recipient, &100, &expiry));
    }
    ids
}

fn advance_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|l| {
        l.sequence_number += delta;
    });
}

// ---------------------------------------------------------------------------
// Claim lifecycle
// ---------------------------------------------------------------------------

#[test]
fn claim_transfers_escrow_and_settles() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);
    let token_client = token::Client::new(&fx.env, &fx.token_addr);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    assert_eq!(token_client.balance(&fx.contract_id), 500);

    client.claim_aid(&aid_id, &fx.recipient);

    assert_eq!(token_client.balance(&fx.recipient), 500);
    assert_eq!(token_client.balance(&fx.contract_id), 0);

    let record = client.get_aid(&aid_id).unwrap();
    assert_eq!(record.status, AidStatus::Settled);
}

#[test]
fn second_claim_returns_already_claimed() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    client.claim_aid(&aid_id, &fx.recipient);

    let result = client.try_claim_aid(&aid_id, &fx.recipient);
    assert_eq!(result, Err(Ok(AidError::AlreadyClaimed)));
}

#[test]
fn claim_after_expiry_is_rejected() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);

    advance_ledger(&fx.env, 101);

    let result = client.try_claim_aid(&aid_id, &fx.recipient);
    assert_eq!(result, Err(Ok(AidError::Expired)));
}

#[test]
fn claim_by_wrong_address_is_unauthorized() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);
    let stranger = Address::generate(&fx.env);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);

    let result = client.try_claim_aid(&aid_id, &stranger);
    assert_eq!(result, Err(Ok(AidError::Unauthorized)));
}

#[test]
fn claim_while_paused_is_rejected() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    client.set_paused(&fx.admin, &true);

    let result = client.try_claim_aid(&aid_id, &fx.recipient);
    assert_eq!(result, Err(Ok(AidError::Paused)));
}

#[test]
fn create_aid_rejects_non_positive_amount_and_past_expiry() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    assert_eq!(
        client.try_create_aid(&fx.donor, &fx.recipient, &0, &expiry),
        Err(Ok(soroban_sdk::Error::from_contract_error(
            SharedError::InvalidAmount as u32
        )))
    );

    let past = fx.env.ledger().sequence();
    assert_eq!(
        client.try_create_aid(&fx.donor, &fx.recipient, &100, &past),
        Err(Ok(soroban_sdk::Error::from_contract_error(
            AidError::NotExpiredYet as u32
        )))
    );
}

// ---------------------------------------------------------------------------
// Refunds
// ---------------------------------------------------------------------------

#[test]
fn refund_aid_after_expiry_returns_funds_to_donor() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);
    let token_client = token::Client::new(&fx.env, &fx.token_addr);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    advance_ledger(&fx.env, 101);

    client.refund_aid(&aid_id);

    assert_eq!(token_client.balance(&fx.donor), MINT_AMOUNT);
    assert_eq!(token_client.balance(&fx.contract_id), 0);
    let record = client.get_aid(&aid_id).unwrap();
    assert_eq!(record.status, AidStatus::Refunded);
}

#[test]
fn refund_aid_before_expiry_is_rejected() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);

    let result = client.try_refund_aid(&aid_id);
    assert_eq!(result, Err(Ok(AidError::NotExpiredYet)));
}

#[test]
fn refund_claimed_aid_is_rejected() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    client.claim_aid(&aid_id, &fx.recipient);
    advance_ledger(&fx.env, 101);

    let result = client.try_refund_aid(&aid_id);
    assert_eq!(result, Err(Ok(AidError::AlreadyClaimed)));
}

#[test]
fn refund_refunded_aid_is_rejected() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    advance_ledger(&fx.env, 101);
    client.refund_aid(&aid_id);

    let result = client.try_refund_aid(&aid_id);
    assert_eq!(result, Err(Ok(AidError::AlreadyRefunded)));
}

#[test]
fn refund_by_admin_is_successful() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);
    let token_client = token::Client::new(&fx.env, &fx.token_addr);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &500, &expiry);
    advance_ledger(&fx.env, 101);

    client.refund_aid(&aid_id);

    assert_eq!(token_client.balance(&fx.donor), MINT_AMOUNT);
    let record = client.get_aid(&aid_id).unwrap();
    assert_eq!(record.status, AidStatus::Refunded);
}

// ---------------------------------------------------------------------------
// Single-record queries
// ---------------------------------------------------------------------------

#[test]
fn get_aid_unknown_id_returns_none() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    assert_eq!(client.get_aid(&9_999), None);
}

#[test]
fn get_aid_returns_full_record() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let expiry = fx.env.ledger().sequence() + 100;
    let aid_id = client.create_aid(&fx.donor, &fx.recipient, &250, &expiry);

    let record = client.get_aid(&aid_id).expect("record should exist");
    assert_eq!(record.id, aid_id);
    assert_eq!(record.donor, fx.donor);
    assert_eq!(record.recipient, fx.recipient);
    assert_eq!(record.token, fx.token_addr);
    assert_eq!(record.amount, 250);
    assert_eq!(record.expiry_ledger, expiry);
    assert_eq!(record.status, AidStatus::Pending);
}

#[test]
fn aid_ids_are_unique_and_monotonic() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let ids = create_aids(&fx.env, &client, &fx.donor, &fx.recipient, 5);
    let sorted = ids.clone();
    assert_eq!(ids, sorted);
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i as u64);
    }
}

// ---------------------------------------------------------------------------
// Pagination tests
// ---------------------------------------------------------------------------

#[test]
fn pagination_empty_for_unknown_user() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);
    let stranger = Address::generate(&fx.env);

    let page_donor = client.list_aids_by_donor(&stranger, &0, &10);
    assert_eq!(page_donor.records.len(), 0);
    assert_eq!(page_donor.next_cursor, None);

    let page_recipient = client.list_aids_by_recipient(&stranger, &0, &10);
    assert_eq!(page_recipient.records.len(), 0);
    assert_eq!(page_recipient.next_cursor, None);
}

#[test]
fn pagination_cursor_and_multi_page_traversal() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    let created_ids = create_aids(&fx.env, &client, &fx.donor, &fx.recipient, 5);

    // Page 1: cursor 0, limit 2 -> items 0, 1
    let p1 = client.list_aids_by_donor(&fx.donor, &0, &2);
    assert_eq!(p1.records.len(), 2);
    assert_eq!(p1.records.get(0).unwrap().id, created_ids[0]);
    assert_eq!(p1.records.get(1).unwrap().id, created_ids[1]);
    assert_eq!(p1.next_cursor, Some(2));

    // Page 2: cursor 2, limit 2 -> items 2, 3
    let p2 = client.list_aids_by_donor(&fx.donor, &p1.next_cursor.unwrap(), &2);
    assert_eq!(p2.records.len(), 2);
    assert_eq!(p2.records.get(0).unwrap().id, created_ids[2]);
    assert_eq!(p2.records.get(1).unwrap().id, created_ids[3]);
    assert_eq!(p2.next_cursor, Some(4));

    // Page 3: cursor 4, limit 2 -> item 4, next_cursor None
    let p3 = client.list_aids_by_donor(&fx.donor, &p2.next_cursor.unwrap(), &2);
    assert_eq!(p3.records.len(), 1);
    assert_eq!(p3.records.get(0).unwrap().id, created_ids[4]);
    assert_eq!(p3.next_cursor, None);

    // Past last page: cursor 10 -> empty list, next_cursor None
    let p_past = client.list_aids_by_donor(&fx.donor, &10, &2);
    assert_eq!(p_past.records.len(), 0);
    assert_eq!(p_past.next_cursor, None);
}

#[test]
fn pagination_by_recipient_matches_assigned_aids() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);
    let other_recipient = Address::generate(&fx.env);

    let id1 = client.create_aid(&fx.donor, &fx.recipient, &100, &(fx.env.ledger().sequence() + 1000));
    let id2 = client.create_aid(&fx.donor, &other_recipient, &200, &(fx.env.ledger().sequence() + 1000));
    let id3 = client.create_aid(&fx.donor, &fx.recipient, &300, &(fx.env.ledger().sequence() + 1000));

    let p_rec1 = client.list_aids_by_recipient(&fx.recipient, &0, &10);
    assert_eq!(p_rec1.records.len(), 2);
    assert_eq!(p_rec1.records.get(0).unwrap().id, id1);
    assert_eq!(p_rec1.records.get(1).unwrap().id, id3);
    assert_eq!(p_rec1.next_cursor, None);

    let p_rec2 = client.list_aids_by_recipient(&other_recipient, &0, &10);
    assert_eq!(p_rec2.records.len(), 1);
    assert_eq!(p_rec2.records.get(0).unwrap().id, id2);
    assert_eq!(p_rec2.next_cursor, None);
}

#[test]
fn pagination_max_query_limit_enforced() {
    let fx = setup();
    let client = AidContractClient::new(&fx.env, &fx.contract_id);

    // Create 55 aids (more than MAX_QUERY_LIMIT = 50)
    let _ids = create_aids(&fx.env, &client, &fx.donor, &fx.recipient, 55);

    // Request with limit 100, should be capped at 50
    let page = client.list_aids_by_donor(&fx.donor, &0, &100);
    assert_eq!(page.records.len(), 50);
    assert_eq!(page.next_cursor, Some(50));

    // Next page fetches remaining 5
    let page2 = client.list_aids_by_donor(&fx.donor, &page.next_cursor.unwrap(), &100);
    assert_eq!(page2.records.len(), 5);
    assert_eq!(page2.next_cursor, None);
}
