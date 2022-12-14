use std::{mem::size_of, collections::HashMap};

use crate::*;

pub(crate) fn royalty_to_payout(royalty_percentage: u32, amount_to_pay: Balance) -> U128 {
    U128(royalty_percentage as u128 * amount_to_pay / 10_000u128)
}

// Refund the storage taken up by passed in approved account IDs and send the funds to the passed in account ID
pub(crate) fn refund_approved_account_ids_iter<'a, I>(
    account_id: AccountId,
    approved_account_ids: I,
) -> Promise
where
    I: Iterator<Item= &'a AccountId>,
{
    // get the storage total by going through and summing all the bytes for each approved account IDS
    let storage_released: u64 = approved_account_ids.map(bytes_for_approved_account_id).sum();

    // transfer
    Promise::new(account_id).transfer(Balance::from(storage_released) * env::storage_byte_cost())
}

// Refund a map of approved account IDs and send the funds to the passed in account id
pub(crate) fn refund_approved_account_ids(
    account_id: AccountId,
    approved_account_ids: &HashMap<AccountId, u64>
) -> Promise {
    // Call the refund_approved_account_ids_iter with the approved account IDs as keys
    refund_approved_account_ids_iter(account_id, approved_account_ids.keys())
}

// Used to generate a unique prefix in our storage collections (avoid data collisions)
pub(crate) fn hash_account_id(account_id: &AccountId) -> CryptoHash {
    // get the default hash
    let mut hash = CryptoHash::default();

    // we hash the account id and return it
    hash.copy_from_slice(&env::sha256(account_id.as_bytes()));
    hash
}

// Calculate how many bytes the account ID is taking up
pub(crate) fn bytes_for_approved_account_id(account_id: &AccountId) -> u64 {
    // The extra 4 bytes are coming from Borsh Serialization to store the length of the string
    account_id.as_str().len() as u64 + 4 + size_of::<u64>() as u64
}

//used to make sure the user attached exactly 1 yoctoNEAR
pub(crate) fn assert_one_yocto() {
    assert_eq!(
        env::attached_deposit(),
        1,
        "Requires attached deposit of exactly 1 yoctoNEAR",
    )
}

pub(crate) fn assert_at_least_one_yocto() {
    assert!(
        env::attached_deposit() >=1,
        "Requires attached deposit of at least 1 yoctoNEAR",
    )
}

// refund the initial deposit based on the amount of stroage that was used up
pub(crate) fn refund_deposit(storage_used: u64) {
    // get how much it would cost to store the information
    let required_cost = env::storage_byte_cost() * Balance::from(storage_used);
    // get the attached deposit
    let attached_deposit = env::attached_deposit();

    // Make sure that the attached deposit is greater than or equal to the required cost
    assert!(
        required_cost <= attached_deposit,
        "Must attach {} yoctoNEAR to cover storage",
        required_cost
    );

    // get the refund amount from the attached deposit
    let refund = attached_deposit - required_cost;

    // if the refund is greater than 1 yocto NEAR, we refund the predecessor that amount
    if refund > 1 {
        Promise::new(env::predecessor_account_id()).transfer(refund);
    }
}

impl Contract {
    // add a token to the set of tokens an owner has
    pub(crate) fn internal_add_token_to_owner(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId
    ) {
        // get the set of tokens for the given account
        let mut tokens_set = self.tokens_per_owner.get(account_id).unwrap_or_else(|| {
            // if the account doesn't have any tokens, we create a new unordered set
            UnorderedSet::new(
                StorageKey::TokenPerOwnerInner {
                    // We get a new unique prefix for the collection
                    account_id_hash: hash_account_id(account_id),
                }
                .try_to_vec()
                .unwrap()
            )
        });

        // We insert the token ID into the set
        tokens_set.insert(token_id);

        // we insert that set for the given account ID
        self.tokens_per_owner.insert(account_id, &tokens_set);
    }

    pub(crate) fn internal_remove_token_from_owner(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId
    ) {
        // get the set of tokens that the owner has
        let mut tokens_set = self.tokens_per_owner
            .get(account_id)
            .expect("Token should be owned by the sender");
        
        // Remove the token_id from the set of tokens
        tokens_set.remove(token_id);

        // if the token set is now empty, we remove the owner from the tokens_per_ownder collection
        if tokens_set.is_empty() {
            self.tokens_per_owner.remove(account_id);
        } else {
            // if the token set is not empty, insert it back for the account ID
            self.tokens_per_owner.insert(account_id, &tokens_set);
        }
    }

    pub(crate) fn internal_transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        token_id: &TokenId,
        approval_id: Option<u64>,
        memo: Option<String>
    ) -> Token {
        // Get the token object by passing in the token_id
        let token = self.tokens_by_id.get(token_id).expect("Token does not exist");

        // Check to see if token is Badge type, if it is panic
        let token_metadata = self.token_metadata_by_id.get(token_id).expect("Token does not exist");
        
        match token_metadata.token_type.unwrap_or(TokenType::Content) {
            TokenType::Content => {},
            TokenType::Badge => {
                // Badge is not allowed to be transferred
                panic!("Your minted badge is not allowed to be transferred. Please contact Educoin for more information.")
            } 
        }

        // If the sender doesnt equal the owner, chceck if the send is in the approval list
        if sender_id != &token.owner_id {
            // if the token's approved account IDs doesn't contain the sender, panic
            if !token.approved_account_ids.contains_key(sender_id) {
                env::panic_str("Unauthorized");
            }
            // If they included an approval_id, check if the sender's actual approval_id is
            // the same as the one included
            if let Some(enforced_approval_id) = approval_id {
                // get the actual approval ID
                let actual_approval_id = token
                    .approved_account_ids
                    .get(sender_id)
                    .expect("Sender is not approved account");
                // make sure that the actual approval id is the same as the one provided
                assert_eq!(
                    actual_approval_id, &enforced_approval_id,
                    "The actual approval_id {} is ddiferent from the given approval_id {}",
                    actual_approval_id, enforced_approval_id
                );
            }
        }

        // make sure that the sender isn't sending the token to themselves
        assert_ne!(
            &token.owner_id, receiver_id,
            "The token owner and the receiver should be different"
        );

        // remove the token from its current owner's set
        self.internal_remove_token_from_owner(&token.owner_id, token_id);

        // add the token to the receiver_id's set
        self.internal_add_token_to_owner(receiver_id, token_id);

        // create a new token struct
        let new_token = Token {
            owner_id: receiver_id.clone(),
            // reset the approval account IDS
            approved_account_ids: Default::default(),
            next_approval_id: token.next_approval_id,
            // copy over the royalties from the previous token
            royalty: token.royalty.clone()
        };
        // insert new token into the tokens_by_id, replacing the old entry
        self.tokens_by_id.insert(token_id, &new_token);

        // if there was some memo attached, we log it
        if let Some(memo) = memo.as_ref() {
            env::log_str(&format!("Memo: {}", memo).to_string());
        }

        // default the authorized ID to be None for the logs
        let mut authorized_id = None;
        // if the approval ID was provided, set the authorized ID equal to the sender
        if approval_id.is_some() {
            authorized_id = Some(sender_id.to_string());
        }

        // construct the transfer log as per the event standard
        let nft_transfer_log: EventLog = EventLog { 
            standard: NFT_STANDARD_NAME.to_string(), 
            version: NFT_METADATA_SPEC.to_string(), 
            event: EventLogVariant::NftTransfer(vec![NftTransferLog {
                authorized_id,
                old_owner_id: token.owner_id.to_string(),
                new_owner_id: receiver_id.to_string(),
                token_ids: vec![token_id.to_string()],
                memo
            }]) 
        };

        // log the serialized json
        env::log_str(&nft_transfer_log.to_string());

        // return the previous token object that was transferred
        token
    }
}