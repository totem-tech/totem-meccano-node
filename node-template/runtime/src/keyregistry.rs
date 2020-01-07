// Copyright 2019 Chris D'Costa
// This file is part of Totem Live Accounting.
// Author Chris D'Costa email: chris.dcosta@totemaccounting.com

// Totem is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Totem is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Totem.  If not, see <http://www.gnu.org/licenses/>.

use parity_codec::{Decode, Encode};
use primitives::{ed25519, H256};
use rstd::prelude::*;
use runtime_primitives::traits::Verify;
use support::{decl_event, decl_module, decl_storage, StorageMap, dispatch::Result, ensure};
use system::{self, ensure_signed};
use runtime_io::{blake2_128, blake2_256};

// bring in Nacl encryption
use sodalite::{box_, box_keypair_seed, BoxPublicKey, BoxSecretKey, BoxNonce};

pub trait Trait: system::Trait + timestamp::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type EncryptNonce = BoxNonce;

// pub type RawNonce = u64;


pub type EncryptPublicKey = H256; //32 bytes Hex
pub type EphemeralSecretSeed = H256; //32 bytes Hex

pub type UserNameHash = H256;
pub type RandomHashedData = H256;

pub type Ed25519signature = ed25519::Signature; //AuthoritySignature
pub type SignedBy = <Ed25519signature as Verify>::Signer; //AuthorityId

pub type Data = Vec<u8>;

pub type EphemeralPublicKey = BoxSecretKey; // generated internally
pub type EphemeralSecretKey = BoxSecretKey; // generated internally

// Tuple for verification data
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct EncryptedVerificationData(Data, EncryptPublicKey);

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, Default)]
pub struct SignedData<UserNameHash, EncryptPublicKey, SignedBy, EncryptNonce> {
    user_hash: UserNameHash,
    pub_enc_key: EncryptPublicKey,
    pub_sign_key: SignedBy,
    nonce: EncryptNonce,
}

decl_storage! {
    trait Store for Module<T: Trait> as KeyRegistryModule {
        UserKeysVerified get(user_keys_verified): map UserNameHash => Option<bool>;
        PublicKeyEnc get(public_key_enc): map UserNameHash => Option<EncryptPublicKey>;
        TempPublicKeyEnc get(temp_public_key_enc): map UserNameHash => Option<EncryptPublicKey>;
        PublicKeySign get(public_key_sign): map UserNameHash => Option<SignedBy>;
        TempPublicKeySign get(temp_public_key_sign): map UserNameHash => Option<SignedBy>;
        VerificationData get(verification_data): map UserNameHash => Option<EncryptedVerificationData>;
        // OldNonce get(old_nonce): Option<RawNonce>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        
        // Chat User registers (untrusted/unvalidated) encryption and signing keys
        fn register_keys(
            origin,
            user_hash: UserNameHash,
            pub_enc_key: EncryptPublicKey,
            pub_sign_key: SignedBy, //
            // data: Vec<u8>,
            nonce: EncryptNonce,
            signature: Ed25519signature // detached signature
        ) -> Result {
            // check that the transaction is signed
            let user = ensure_signed(origin)?;
            // if the usernamehash exists, compare keys
            let transaction_data = SignedData {
                user_hash: user_hash.clone(),
                pub_enc_key: pub_enc_key.clone(),
                pub_sign_key: pub_sign_key.clone(),
                nonce: nonce.into(),
            };
            
            // check if this user has submitted keys before
            match Self::user_keys_verified(user_hash.clone()) {
                Some(true) => {
                    // The existing key is verified, but this time it may be a replacement of the key(s).
                    // Get both keys from storage or error.
                    let old_enc_key = Self::public_key_enc(&user_hash).ok_or("Storage Read Error: cannot get encryption key, or key is not verified")?; 
                    let old_sign_key = Self::public_key_sign(&user_hash).ok_or("Storage Read Error: cannot get signature key, or key is not verified")?; 
                    
                    let transaction_data_clone = transaction_data.clone(); 
                    let encoded_data: Vec<u8> = transaction_data_clone.encode(); 
                    // If the encryption key or the signing key are not the same
                    if old_enc_key != transaction_data.pub_enc_key || old_sign_key != transaction_data.pub_sign_key {
                        // The keys are different, 
                        // Check that the NEW data is signed by the OLD signature key
                        ensure!(signature.verify(&encoded_data[..], &old_sign_key), "Invalid signature for this key");
                        
                        // Store keys in temp space pending verification
                        if old_enc_key != transaction_data.pub_enc_key {
                            <TempPublicKeyEnc<T>>::take(&user_hash);
                            <TempPublicKeyEnc<T>>::insert(&user_hash, &transaction_data.pub_enc_key);
                        };
                        
                        if old_sign_key != transaction_data.pub_sign_key {
                            <TempPublicKeySign<T>>::take(&user_hash);
                            <TempPublicKeySign<T>>::insert(&user_hash, &transaction_data.pub_sign_key);
                            
                        };
                        
                        // generate 128bit verification data
                        let message = Self::pseudo_random_value(&transaction_data);
                        
                        // Generate ephemeral keys for symmetric encryption
                        let mut ephemeral_public_key: EphemeralPublicKey = Default::default();
                        let mut ephemeral_secret_key: EphemeralSecretKey = Default::default();
                        
                        let ephemeral_secret_seed = <system::Module<T>>::random_seed().using_encoded(blake2_256);
                        
                        box_keypair_seed(&mut ephemeral_public_key, &mut ephemeral_secret_key, &ephemeral_secret_seed);                        
                        
                        // let mut last_nonce: RawNonce = 0;    
                        
                        // // get next nonce
                        // if <OldNonce<T>>::exists() {
                        //     last_nonce = Self::old_nonce().ok_or("Storage Read Error: cannot get nonce").unwrap();
                        // };
                        
                        // this is a dummy placeholder until we work out how to increment nonce
                        let mut last_nonce_24: EncryptNonce = [0u8; 24];
                        
                        // create full message with information that is required to 
                        // prove that the decrypter is the owner. 
                        // the ephemeral secret key  will not be stored
                        // therefore in order for the genuine owner to prove ownership they must decrypt
                        // and sign what they have decrypted with the signature key provided.
                        // The ephemeral secret key is then revealed and is used a second time to 
                        // prove the contents of the validation data directly.
                        // upon success the runtime can be sure that the encryption key owner was able to
                        // discover the ephemeral secret key.
                         
                        let data_to_encrypt = (message, &ephemeral_secret_key).encode();
                        
                        // Convert from H256 to [u8; 32]. Might need dereferencing in other contexts
                        let external_pub_key: &BoxPublicKey  = transaction_data.pub_enc_key.as_fixed_bytes();

                        // initialise ciphertext with a default value 
                        let mut cipher_text = [0u8];

                        // Encrypt data returning cipher_text
                        box_(&mut cipher_text, &data_to_encrypt, &last_nonce_24, external_pub_key, &ephemeral_secret_key);
                        // should handle error on encryption here.
                        
                        // parse cipher to Vec<u8> string for storage and reading in UI
                        let cipher: Vec<u8> = cipher_text.to_vec();

                        // convert from raw public key to UI readable public key
                        let pubkey = ed25519::Public::from_raw(ephemeral_public_key).0.into();

                        let verify_this = EncryptedVerificationData(cipher, pubkey);
                        
                        // Store cipher against user id hash 
                        // remove any existing data
                        <VerificationData<T>>::take(&user_hash);
                        // insert (or replace)
                        <VerificationData<T>>::insert(&user_hash, verify_this);
                        
                    }; // if the keys are the same, do nothing    
                    
                    
                }, 
                Some(false) => return Err("The existing key hasn't yet been formally validated by the key owner"),
                None => {
                    // This is a new set of keys
                    // Store keys in temp space pending verification
                    <TempPublicKeyEnc<T>>::insert(&user_hash, &transaction_data.pub_enc_key);
                    <TempPublicKeySign<T>>::insert(&user_hash, &transaction_data.pub_sign_key);
                    
                    // generate 128bit verification data
                    let random_validation_key = Self::pseudo_random_value(&transaction_data);
                    // encrypt verification data
        
        
                    // store verification data

                }  
            } //match
            
            Ok(())
        } 
            
    }
        
}


decl_event!(
    pub enum Event<T>
    where
    AccountId = <T as system::Trait>::AccountId,
    Hash = <T as system::Trait>::Hash,
    {
        SubmitedKeys(AccountId, Hash),
    }
);

impl<T: Trait> Module<T> {
    fn pseudo_random_value(data: &SignedData<UserNameHash, EncryptPublicKey, SignedBy, EncryptNonce>) -> [u8; 16] {
        let input = (
            <timestamp::Module<T>>::get(),
            <system::Module<T>>::random_seed(),
            data,
            <system::Module<T>>::extrinsic_index(),
            <system::Module<T>>::block_number(),
        );
        return input.using_encoded(blake2_128);
    }

    // fn encrypt_pseudo_random_value(data: [u8; 16], external_public_key: EncryptPublicKey) -> Vec<u8> {
        // let mut last_nonce: EncryptNonce = 0;
        // if <OldNonce<T>>::exists() {
        //     last_nonce = Self::old_nonce().ok_or("Storage Read Error: cannot get nonce").unwrap();
        // };

        // let cipher  = EncryptedVerificationData(data.to_vec(), last_nonce.clone()).encode(); 

        // // generate ephemeral secret key
        // let mut ephemeral_secret_seed = [0u8; 32];
        
        // let seed_bytes = <system::Module<T>>::random_seed();
        
        // // seed_bytes.fill_bytes(&mut ephemeral_secret_seed[..]);
        
        // let ephemeral_secret_key = calc_dhshared_key(&external_public_key, &ephemeral_secret_seed);
        
        
        // take ephemeral secret key and external public key, and generate Diffie-Hellman shared key


        // add the ephemeral secret key to the data and a nonce & encrypt;

        // store the encrypted cipher against the user name hash 

        // when the user decrypts, they sign the data, nonce and secret key back to runtime

        // runtime checks signature, re-encrypts using the supplied ephemeral secret key and if the values matches, it permanently stores the original keys, removes the temp, cleans up validation 






        
        // <OldNonce<T>>::put(&last_nonce + 1);

    //     return true;
    // }

    fn store_validation() -> bool {
        // EncryptedVerificationData(Data, EncryptNonce);
        
        return true;
    }
}


// In the front end we send the data(all elements) attached to the signature as a string Vec<u8>: Easy
// using sr_io::ed25519_verify(signature, message, publicsignigkey) we can return TRUE (vaerified signature) or FALSE (not verified signature)
// What we are missing is the validation of the encryption key... something like
// Client sends
// random data_,
// encrypted_data_1,
// enc_pub_key
// The runtime validates this by ->
// encrypting data_ (to encrypted_data_2),
// comparing encrypted_data_2 to encrypted_data_1 returniong TRUE or FALSE.

// However, for expediency (Yikes!) we can assert that the signer is claiming this public encryption key. THIS IS NOT IDEAL.

// On this basis we can encodesend the

// // check a message signature. returns true if signed by that authority.
// fn check_message_sig<B: Codec, H: Codec>(
    // 	message: Message<B, H>,
    // 	signature: &Signature,
    // 	from: &AuthorityId
    // ) -> bool {
        // 	let msg: Vec<u8> = message.encode();
        // 	sr_io::ed25519_verify(&signature.0, &msg, from)
        //}
        