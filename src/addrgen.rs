use crate::input::NUMBER_OF_ADDRESSES;
use arbitrary::Unstructured;
use ed25519_dalek::SigningKey;
use soroban_sdk::testutils::arbitrary::arbitrary;
use soroban_sdk::xdr::{
    AccountEntry, AccountEntryExt, AccountId, AlphaNum4, AssetCode4, Hash, LedgerEntry,
    LedgerEntryData, LedgerEntryExt, LedgerKey, LedgerKeyAccount, LedgerKeyTrustLine, PublicKey,
    ScAddress, SequenceNumber, Signer, SignerKey, Thresholds, TrustLineAsset, TrustLineEntry,
    TrustLineEntryExt, TrustLineFlags, Uint256,
};
use soroban_sdk::{Address, Env, TryFromVal};
use std::rc::Rc;
use std::vec::Vec as RustVec;

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct AddressGenerator {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(u64::MIN..=u64::MAX - NUMBER_OF_ADDRESSES as u64))]
    pub address_seed: u64,
    pub address_types: [AddressType; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub enum AddressType {
    Account,
    Contract,
}

pub struct TestSigner {
    pub address: Address,
    pub key: Option<SigningKey>,
}

impl AddressGenerator {
    pub fn generate_signers(&self, env: &Env) -> RustVec<TestSigner> {
        self.generate_signers_with_bytes(env)
            .into_iter()
            .map(|(a, _)| a)
            .collect()
    }

    fn generate_signers_with_bytes(&self, env: &Env) -> RustVec<(TestSigner, [u8; 32])> {
        let mut signers = RustVec::<(TestSigner, [u8; 32])>::new();

        // fixme seed of 0 or 1 seems to generate bogus contract addresses
        for i in 0..NUMBER_OF_ADDRESSES {
            let seed = self
                .address_seed
                .checked_add(i as u64)
                .expect("Overflow")
                .to_be_bytes();
            let signer_bytes: [u8; 32] = [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, seed[0],
                seed[1], seed[2], seed[3], seed[4], seed[5], seed[6], seed[7],
            ];

            let test_signer = match self.address_types[i] {
                AddressType::Account => {
                    let signing_key = SigningKey::from_bytes(&signer_bytes);
                    let verifying_key = signing_key.verifying_key().to_bytes();

                    let account_id =
                        AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(verifying_key)));
                    let sc_address = ScAddress::Account(account_id);
                    let address = Address::try_from_val(env, &sc_address).unwrap();
                    let test_signer = TestSigner {
                        address,
                        key: Some(signing_key),
                    };

                    test_signer
                }
                AddressType::Contract => {
                    let address =
                        Address::try_from_val(env, &ScAddress::Contract(Hash(signer_bytes)))
                            .unwrap();
                    let test_signer = TestSigner { address, key: None };

                    test_signer
                }
            };

            signers.push((test_signer, signer_bytes));
        }

        signers
    }

    pub fn setup_account_storage(&self, env: &Env) {
        let signers_n_bytes = self.generate_signers_with_bytes(&env);
        signers_n_bytes.iter().for_each(|(signer, bytes)| {
            let sc_addr = ScAddress::try_from(signer.address.clone()).unwrap();
            match sc_addr {
                ScAddress::Account(account_id) => {
                    let signing_key = SigningKey::from_bytes(bytes);
                    create_default_account(&env, &account_id, vec![(&signing_key, 100)]);
                    create_default_trustline(&env, &account_id);
                }
                ScAddress::Contract(_) => {}
            }
        });
    }
}

fn create_default_account(env: &Env, account_id: &AccountId, signers: Vec<(&SigningKey, u32)>) {
    let key = LedgerKey::Account(LedgerKeyAccount {
        account_id: account_id.clone(),
    });
    let mut acc_signers = vec![];
    for (signer, weight) in signers {
        acc_signers.push(Signer {
            key: SignerKey::Ed25519(Uint256(signer.verifying_key().to_bytes())),
            weight,
        });
    }

    let ext = AccountEntryExt::V0;
    let acc_entry = AccountEntry {
        account_id: account_id.clone(),
        balance: 10_000_000,
        seq_num: SequenceNumber(0),
        num_sub_entries: 0,
        inflation_dest: None,
        flags: 0,
        home_domain: Default::default(),
        thresholds: Thresholds([1, 0, 0, 0]),
        signers: acc_signers.try_into().unwrap(),
        ext,
    };

    env.host()
        .with_mut_storage(|storage| {
            storage.put(
                &Rc::new(key),
                &Rc::new(LedgerEntry {
                    last_modified_ledger_seq: 0,
                    data: LedgerEntryData::Account(acc_entry),
                    ext: LedgerEntryExt::V0,
                }),
                None,
                soroban_env_host::budget::AsBudget::as_budget(env.host()),
            )
        })
        .expect("ok");
}

fn create_default_trustline(env: &Env, account_id: &AccountId) {
    // This is deterministically generated by Env::register_stellar_asset_contract,
    // but could change if usage of the Env changes during the setup phase of the fuzzer.
    let issuer_bytes: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ];

    let issuer = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(issuer_bytes)));
    let asset = TrustLineAsset::CreditAlphanum4(AlphaNum4 {
        asset_code: AssetCode4([b'a', b'a', b'a', 0]),
        issuer: issuer,
    });

    let key = LedgerKey::Trustline(LedgerKeyTrustLine {
        account_id: account_id.clone(),
        asset: asset.clone(),
    });

    let flags =
        TrustLineFlags::AuthorizedFlag as u32 | TrustLineFlags::TrustlineClawbackEnabledFlag as u32;

    let ext = TrustLineEntryExt::V0;

    let trustline_entry = TrustLineEntry {
        account_id: account_id.clone(),
        asset,
        balance: 0,
        limit: i64::MAX,
        flags,
        ext,
    };

    env.host()
        .with_mut_storage(|storage| {
            storage.put(
                &Rc::new(key),
                &Rc::new(LedgerEntry {
                    last_modified_ledger_seq: 0,
                    data: LedgerEntryData::Trustline(trustline_entry),
                    ext: LedgerEntryExt::V0,
                }),
                None,
                soroban_env_host::budget::AsBudget::as_budget(env.host()),
            )
        })
        .expect("ok");
}
