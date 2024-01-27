#![no_main]

use libfuzzer_sys::{fuzz_target, Corpus};
use soroban_sdk::{Address, Env, Error, InvokeError, String, TryFromVal, Val};
use soroban_token_fuzzer::*;

use mobloom::contract::Token;
use mobloom::TokenClient;

// This is the entrypoint.
//
// `Input` is randomly generated by the fuzzer and interpreted
// by the `fuzz_token` function.
//
// The `Corpus` return type is used by the fuzzer to communicate
// the results of fuzzing back to the fuzzer, particulary to
// instruct the fuzzer that the `Input` case was unusable
// (for various reasons).
fuzz_target!(|input: Input| -> Corpus {
    // Each token needs to construct its own `Config` by passing
    // to `contract` a type that implements `ContractTokenOps`.
    let config = Config::contract(TokenOps);
    // Run the fuzzer.
    fuzz_token(config, input)
});

// Implements `ContractTokenOps`
struct TokenOps;

// Implements `TokenAdminClient`
struct AdminClient<'a> {
    client: TokenClient<'a>,
}

impl ContractTokenOps for TokenOps {
    /// Register the contract with the environment and perform
    /// contract-specific one-time initialization.
    ///
    /// This function will be called once.
    fn register_contract_init(&self, env: &Env, admin: &Address) -> Address {
        let token_contract_id = env.register_contract(None, Token);

        let admin_client = TokenClient::new(&env, &token_contract_id);
        let r = admin_client.try_initialize(
            &admin,
            &10,
            &String::from_str(&env, "token"),
            &String::from_str(&env, "TKN"),
        );

        assert!(r.is_ok());

        token_contract_id
    }

    /// Register the contract with the environment.
    ///
    /// This will be called on all subsequent transactions
    /// after the first, i.e. every time time is advanced
    /// and the `Env` is recreated.
    fn reregister_contract(&self, env: &Env, token_contract_id: &Address) {
        env.register_contract(Some(token_contract_id), Token);
    }

    /// Create an admin client.
    fn new_admin_client<'a>(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) -> Box<dyn TokenAdminClient<'a> + 'a> {
        Box::new(AdminClient {
            client: TokenClient::new(&env, &token_contract_id),
        })
    }
}

impl<'a> TokenAdminClient<'a> for AdminClient<'a> {
    fn try_mint(
        &self,
        to: &Address,
        amount: &i128,
    ) -> Result<Result<(), <() as TryFromVal<Env, Val>>::Error>, Result<Error, InvokeError>> {
        self.client.try_mint(to, amount)
    }
}
