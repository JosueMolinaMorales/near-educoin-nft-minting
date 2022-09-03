use crate::*;

#[near_bindgen]
impl Contract {
    // Query for the total supply of NFTs on the contract
    pub fn nft_total_supply(&self) -> U128 {
        U128(self.token_metadata_by_id.len() as u128)
    }

    // Query for nft tokens on the contract regardless of the onwer using pagination
    pub fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<JsonToken> {
        let start = u128::from(from_index.unwrap_or(U128(0)));

        self.token_metadata_by_id.keys()
            .skip(start as usize)
            .take(limit.unwrap_or(50) as usize)
            .map(|token_id| self.nft_token(token_id.clone()).unwrap())
            .collect()
    }

    // Get the total supply of NFTs for a given owner
    pub fn nft_supply_for_owner(
        &self,
        account_id: AccountId
    ) -> U128 {
        //get the set of tokens for the passed in owner
        let tokens_for_owner_set = self.tokens_per_owner.get(&account_id);

        // if there is some set of tokens, return the length
        if let Some(tokens_for_owner_set) = tokens_for_owner_set {
            U128(tokens_for_owner_set.len() as u128)
        } else {
            // If there isnt a set of tokens for passed in accountid, return 0
            U128(0)
        }
    }

    // Query for all the tokens for an owner
    pub fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>
    ) -> Vec<JsonToken> {
        // Get the set of tokens for the passed in owner
        let tokens_for_owner_set = self.tokens_per_owner.get(&account_id);
        // if there is some set of tokens, we'll set the tokens variable equal to that set
        let tokens = if let Some(tokens_for_owner_set) = tokens_for_owner_set {
            tokens_for_owner_set
        } else {
            // if there is no set of tokens, well simply return an empty vector
            return vec![];
        };

        // start pagination - if we have a from_index use it otherwise start at 0
        let start = u128::from(from_index.unwrap_or(U128(0)));

        // iterate through the keys vector
        tokens.iter()
            // skip to the index we specified in the start variable
            .skip(start as usize)
            // take the first "limit" elelemts in the vector. If we didn't specify a limit, use 50
            .take(limit.unwrap_or(50) as usize)
            // we'll map the token Ids which are string into Json tokens
            .map(|token_id| self.nft_token(token_id.clone()).unwrap())
            // since we turned the keys into an iterator, turn it back
            .collect()
    }
}