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


/// This is the BoXKeyS Protocol for a public key server on a substrate based blockchain
/// Blockchainisation of X25519 Key Server 
/// Authored by Chris D'Costa.

/// The use case is that a key holder is publicly claiming ownership of X25519 public encryption and signing keys
/// However to do so the claimant must also prove that they hold the associated private keys.

/// By having a protocol that independently determines the validity, without requiring a trusted third-party
/// to authenticate the keys is useful because there is currently no common globally available service for this
/// that doesn't rely on third parties. 

/// A simple lookup against a unique ID provides the authenticated public keys for 
/// anyone who wants to encrypt a message to, or validate the signature of another party.  

/// The service can be used out of the box [pun intended] on Totem and any other Substrate chain.

/// The blockchain runtime provides the mechanism for self-verifying ownership using the following procedure: 

/// 1. The claimant submits a hash of unique identifying information, the public signature and encryption keys for which they claim to hold the 
///    the associated secret keys and a message signature which has signed an array of aforementioned data.
///    NOTE: the signature is not necessarily generated from the same key that signs the transaction, meaning any valid identity on Substrate 
///    can submit a claim to keys paying the relevant fees. 
///    Because any identity can sign the transaction, we still need to validate the additional signature provided against the 
///    public signature key that was also provided.
///
/// 2. The runtime checks if this is a new set of keys or a replacement to an existing set. In any case it will generate
///    a random set of data to be encrypted against the provided public encryption key.
///
/// 3. The runtime generates an ephemeral (one-time use) secret key, used to encrypt the data to the provided public encryption key. 
///    The ephemeral key is prepended to the data to be encrypted and then everything is encrypted and stored on chain. 
/// 
/// 4. Although the hash of the identifying userid is public (and therefore can be used to monitor blockchain storage), only the valid holder of the 
///    decryption keys can decrypt the cipher stored on chain. Once decrypted, technically the ephemeral secret key is revealed to the holder of the sencryption key pair.
/// 
/// 5. The holder of the decrypted data is then required to sign the decrypted data with the signature keys that they are also claiming, and send the resulting 
///    signature along with the decrypted data as a transaction back to the blockchain runtime.
/// 
/// 6. As the runtime did not store the unencrypted ephemeral secret key, receiving this information should permit the runtime to regenrate the cipher.  
///    This however by itself does not prove that the sender of the transaction is in possession of the secret encryption key associated with the claimed public encryption key.
///    They must also sign the revealed ephemeral secret key with the claimed public signature key. Only if both these are fulfilled can the keys be considered
///    authenticated.
///
/// 7. This process is identical for replacing the keys, with the added exception that the keys must be signed by the previous signature key.  

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
pub type EncryptPublicKey = H256; //32 bytes Hex

pub type UserNameHash = H256;

pub type Ed25519signature = ed25519::Signature; //AuthoritySignature
pub type SignedBy = <Ed25519signature as Verify>::Signer; //AuthorityId

pub type Data = Vec<u8>;

type EphemeralPublicKey = BoxSecretKey; // generated internally
type EphemeralSecretKey = BoxSecretKey; // generated internally

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
struct PreEncryptionData<EphemeralSecretKey, Data> {
    key: EphemeralSecretKey,
    data: Data
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct EncryptedVerificationData<EncryptPublicKey, Data> {
    key: EncryptPublicKey,
    data : Data
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, Default)]
struct SignedData<UserNameHash, EncryptPublicKey, SignedBy, EncryptNonce> {
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
        VerificationData get(verification_data): map UserNameHash => Option<EncryptedVerificationData<EncryptPublicKey, Data>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        fn destroy_keys(
            origin,
            signature: Ed25519signature
        ) -> Result {
            // provided you are the owner of the keys you can remove them entirely from storage.
            Ok(())

        }
        
        fn auto_verification(
            origin,
            user_hash: UserNameHash, // hash of unique userid
            decrypted: Vec<u8>, // this is a tuple containing (random_validation_key, &ephemeral_secret_key).encode() 
            signature: Ed25519signature // detached signature of "discovered ephemeral secret key"
        ) -> Result {
            // transaction must be signed
            let _user = ensure_signed(origin)?;

            // have they signed the decrypted_data with the correct public key? Yes
            let decrypted_data = decrypted.clone(); 

            let temp_sign_key = Self::temp_public_key_sign(&user_hash).ok_or("Storage Read Error: cannot get signature key")?; 
            ensure!(signature.verify(&decrypted_data[..], &temp_sign_key), "Invalid signature for this key");
            
            // grab the claimed encryption public key from temp storage
            let temp_encrypt_key = Self::temp_public_key_enc(&user_hash).ok_or("Storage Read Error: cannot get encryption key")?; 

            // grab the verification data
            let data_to_compare = Self::verification_data(&user_hash).ok_or("Storage Read Error: cannot get verification data")?; 
            
            // grab the revealed ephemeral secret key
            let unwrapped_data: PreEncryptionData<EphemeralSecretKey, Data> = PreEncryptionData::decode(&mut &decrypted[..]).ok_or("Error parsing the data sent for validation")?;
           
            // Now check that the data supplied can create the correct cipher as stored
            // we should receive the data already encoded, so no need to do anything special
            let data_to_encrypt = decrypted.clone();

            // Convert from H256 to [u8; 32]. Might need dereferencing in other contexts
            let external_pub_key: &BoxPublicKey  = temp_encrypt_key.as_fixed_bytes();

            // this is a dummy placeholder nonce
            let nonce_24: EncryptNonce = [0u8; 24];

            // initialise ciphertext with a default value 
            let mut cipher_text = [0u8];
        
            // Re encrypt the supplied data returning cipher_text, which will be compared to the stored version
            match box_(&mut cipher_text, &data_to_encrypt, &nonce_24, external_pub_key, &unwrapped_data.key) {
                Err(_e) => return Err("Encryption failed."),
                _ => ()
            };

            // compare newly processes cipher to stored cipher, if they agree we have a match!
            let cipher_to_compare = data_to_compare.data;
            match cipher_text.to_vec() {
                cipher_to_compare => {
                    //if we get this far then the data was decrypted by the owner of the encryption key, 
                    // and it was signed by the owner of the signature key
                    
                    // mark the keys as veriffed
                    match Self::set_verification_state(user_hash, true) {
                        Err(_e) => return Err("Failed to store the verification state"),
                        _ => (),
                    }
                    // move the keys to the verified storage
                    
                    
                    
                    
                    
                    
                    // remove the keys fro the temp storage
                    match Self::delete_temp_keys(user_hash) {
                        Err(_e) => return Err("Error removing temp keys"),
                        _ => return Ok(()),
                    };
                },
                _ => return Err("There was an error authenticating the supplied data"),
            };

            Ok(())

        }
        
        // Chat User registers (untrusted/unvalidated) encryption and signing keys
        fn register_keys(
            origin,
            user_hash: UserNameHash, // hash of unique userid
            pub_enc_key: EncryptPublicKey, // master public encryption key associated with chat user
            pub_sign_key: SignedBy, // master public signing key associated with chat user
            nonce: EncryptNonce, // just a nonce generated in the UI
            signature: Ed25519signature // detached signature
        ) -> Result {
            
            // check that the transaction is signed
            let _user = ensure_signed(origin)?;
            // if the usernamehash exists, compare keys
            
            // TODO Errors can occur here!!!! Need to validate inputs.
            let transaction_data = SignedData {
                user_hash: user_hash.clone(),
                pub_enc_key: pub_enc_key.clone(),
                pub_sign_key: pub_sign_key.clone(),
                nonce: nonce.into(), // declared in UI as Vec<u8> could this cause an overflow error?  
            };
            
            // check if this user has submitted keys verified keys before.
            match Self::user_keys_verified(user_hash.clone()) {
                Some(true) => {
                    // The existing key is verified, but this time it may be a replacement of the key(s).
                    // Get both keys from storage or error.
                    let old_enc_key = Self::public_key_enc(&user_hash).ok_or("Storage Read Error: cannot get encryption key, or key is not verified")?; 
                    let old_sign_key = Self::public_key_sign(&user_hash).ok_or("Storage Read Error: cannot get signature key, or key is not verified")?; 
                    
                    let transaction_data_clone = transaction_data.clone(); 
                    let encoded_data: Vec<u8> = transaction_data_clone.encode(); 
                    // If the encryption key or the signing key are not the same as already stored
                    if old_enc_key != transaction_data.pub_enc_key || old_sign_key != transaction_data.pub_sign_key {
                        // The keys are different, 
                        // Check that the NEW data is signed by the OLD signature key
                        ensure!(signature.verify(&encoded_data[..], &old_sign_key), "Invalid signature for this key");
                        
                        // remove and replace keys                        
                        match Self::delete_state_and_temp_keys(user_hash) {
                            Err(_e) => return Err("Failed to remove all keys"),
                            _ => (),
                        };

                        
                        // Store keys in temp space pending verification. It is necessary to do this now.
                        // If a later process fails this will be replaced anyway.
                        if old_enc_key != transaction_data.pub_enc_key {
                            <TempPublicKeyEnc<T>>::insert(&user_hash, &transaction_data.pub_enc_key);
                        };
                        
                        if old_sign_key != transaction_data.pub_sign_key {
                            <TempPublicKeySign<T>>::insert(&user_hash, &transaction_data.pub_sign_key);
                        };
                        
                        match Self::set_generated_verification_data(transaction_data) {
                            Err(_e) => return Err("Failed to store verification data."),
                            _ => ()
                        };
                        
                        // set the verification status to false.
                        match Self::set_verification_state(user_hash, false) {
                            Err(_e) => return Err("Failed to store the verification state"),
                            _ => (),
                        };

                    }; // if the keys are the same, do nothing    
                    
                    
                }, 
                Some(false) => return Err("The existing key hasn't yet been formally validated by the key owner"),
                None => {
                    // This is a first set of keys
                    // Store keys in temp space pending verification
                    <TempPublicKeyEnc<T>>::insert(&user_hash, &transaction_data.pub_enc_key);
                    <TempPublicKeySign<T>>::insert(&user_hash, &transaction_data.pub_sign_key);

                    match Self::set_generated_verification_data(transaction_data) {
                        Err(_e) => return Err("Failed to store verification data."),
                        _ => ()
                    }

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
    fn get_pseudo_random_value(data: &SignedData<UserNameHash, EncryptPublicKey, SignedBy, EncryptNonce>) -> [u8; 16] {
        let input = (
            <timestamp::Module<T>>::get(),
            <system::Module<T>>::random_seed(),
            data,
            <system::Module<T>>::extrinsic_index(),
            <system::Module<T>>::block_number(),
        );
        return input.using_encoded(blake2_128);
    }

    fn set_verification_state(user_hash: UserNameHash, state: bool) -> Result {
        <UserKeysVerified<T>>::insert(&user_hash, state);

        Ok(())
    }
    
    fn delete_state_and_temp_keys(user_hash: UserNameHash) -> Result {        
        <UserKeysVerified<T>>::take(&user_hash);
        match Self::delete_temp_keys(user_hash) {
            Err(_e) => return Err("Error removing temp keys"),
            _ => return Ok(()),
        };
    }
    
    fn delete_temp_keys(user_hash: UserNameHash) -> Result {
        <TempPublicKeyEnc<T>>::take(&user_hash);
        <TempPublicKeySign<T>>::take(&user_hash);
        Ok(())
    }

    fn set_generated_verification_data(transaction_data: SignedData<UserNameHash, EncryptPublicKey, SignedBy, EncryptNonce>) -> Result {
        // generate 128bit verification data
        let random_validation_key = Self::get_pseudo_random_value(&transaction_data);
        
        // encrypt verification data
    
        // Generate ephemeral keys for symmetric encryption
        let mut ephemeral_public_key: EphemeralPublicKey = Default::default();
        let mut ephemeral_secret_key: EphemeralSecretKey = Default::default();
        
        let ephemeral_secret_seed = <system::Module<T>>::random_seed().using_encoded(blake2_256);
        
        box_keypair_seed(&mut ephemeral_public_key, &mut ephemeral_secret_key, &ephemeral_secret_seed);                        
                                
        // this is a dummy placeholder until we work out how to increment nonce
        let last_nonce_24: EncryptNonce = [0u8; 24];

        // populate struct with data for manipulation.
        let pre_encrytion_data = PreEncryptionData {
            key: &ephemeral_secret_key,
            data: &random_validation_key
        };
        
        let data_to_encrypt = pre_encrytion_data.encode();
    
        // Convert from H256 to [u8; 32]. Might need dereferencing in other contexts
        let external_pub_key: &BoxPublicKey  = transaction_data.pub_enc_key.as_fixed_bytes();
    
        // initialise ciphertext with a default value 
        let mut cipher_text = [0u8];
    
        // Encrypt data returning cipher_text
        match box_(&mut cipher_text, &data_to_encrypt, &last_nonce_24, external_pub_key, &ephemeral_secret_key) {
            Err(_e) => return Err("Encryption failed."),
            _ => ()
        };

        let encrypted_verification_data = EncryptedVerificationData {
            key: ed25519::Public::from_raw(ephemeral_public_key).0.into(), // convert from raw public key to UI readable public key
            data: cipher_text.to_vec(),  // cast cipher_text to Vec<u8> string for storage (and ease of use in UI)
        };
    
        match Self::set_validation_data(transaction_data, encrypted_verification_data) {
            true => return Ok(()),
            false => return Err("Error storing validation data"),
        }
        
    }

    fn set_validation_data(transaction_data: SignedData<UserNameHash, EncryptPublicKey, SignedBy, EncryptNonce>, 
        verify_this: EncryptedVerificationData<EncryptPublicKey, Data>) -> bool {
        
        // EncryptedVerificationData(Data, EncryptNonce);
        <VerificationData<T>>::take(&transaction_data.user_hash);
        // insert (or in the case of new keys, replace)
        <VerificationData<T>>::insert(transaction_data.user_hash, verify_this);
    
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
        