use super::{
    camo_address_tests, get_standard_index, AutoTestUtils, CamoAccountTrait, CamoKeysTrait,
    CamoVersions, CamoViewKeysTrait,
};
use crate::{
    auto_from_impl, base32,
    hashes::{
        blake2b512, blake2b_checksum, blake2b_scalar, get_camo_spend_seed, get_camo_view_seed,
        hazmat::{get_account_scalar, get_account_seed},
    },
    secret, try_compressed_from_slice, try_point_from_slice, version_bits, Account, Block, Key,
    NanoError, Scalar, SecretBytes,
};
use curve25519_dalek::{
    constants::ED25519_BASEPOINT_POINT as G,
    edwards::{CompressedEdwardsY, EdwardsPoint},
};
use std::fmt::Display;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

const ADDRESS_LENGTH: usize = 117;

fn ecdh(key_1: &Scalar, key_2: &EdwardsPoint) -> SecretBytes<32> {
    secret!((key_1 * key_2).compress().to_bytes())
}

/// returns (spend, view)
fn get_partial_keys(view_seed: &SecretBytes<32>, i: u32) -> (Scalar, Scalar) {
    let account_seed = blake2b512(get_account_seed(view_seed, i).as_ref());
    (
        blake2b_scalar(&account_seed.as_ref()[..32]),
        blake2b_scalar(&account_seed.as_ref()[32..64]),
    )
}

fn points_to_account(
    versions: CamoVersions,
    spend: EdwardsPoint,
    view: EdwardsPoint,
) -> CamoAccountV1 {
    let compressed_spend_key = spend.compress();
    let compressed_view_key = view.compress();

    let data = [
        [versions.encode_to_bits()].as_slice(),
        compressed_spend_key.as_bytes(),
        compressed_view_key.as_bytes(),
    ]
    .concat();
    let mut checksum = blake2b_checksum(&data);
    checksum.reverse();

    let mut account = "camo_".to_string();
    let data = [data.as_slice(), &checksum].concat();
    account.push_str(&base32::encode(&data));

    CamoAccountV1 {
        account,
        versions,
        compressed_spend_key,
        compressed_view_key,
        point_spend_key: spend,
        point_view_key: view,
    }
}

fn account_from_data(account: &str, data: &[u8]) -> Result<CamoAccountV1, NanoError> {
    if account.len() != ADDRESS_LENGTH {
        return Err(NanoError::InvalidAddressLength);
    }

    let versions = version_bits!(data[0]);
    let spend_key = &data[1..33];
    let view_key = &data[33..65];
    let checksum = &data[65..70];
    let mut calculated_checksum = blake2b_checksum(&data[..65]);
    calculated_checksum.reverse();

    if checksum != calculated_checksum {
        return Err(NanoError::InvalidAddressChecksum);
    }

    let compressed_spend_key = try_compressed_from_slice(spend_key)?;
    let compressed_view_key = try_compressed_from_slice(view_key)?;

    Ok(CamoAccountV1 {
        account: account.to_string(),
        versions,
        compressed_spend_key,
        compressed_view_key,
        point_spend_key: try_point_from_slice(spend_key)?,
        point_view_key: try_point_from_slice(view_key)?,
    })
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CamoKeysV1 {
    versions: CamoVersions,
    private_spend: Scalar,
    private_view: Scalar,
}
impl CamoKeysTrait for CamoKeysV1 {
    type ViewKeysType = CamoViewKeysV1;
    type AccountType = CamoAccountV1;

    fn from_seed(master_seed: &SecretBytes<32>, i: u32, versions: CamoVersions) -> CamoKeysV1 {
        let master_spend = get_account_scalar(&get_camo_spend_seed(master_seed), 0);
        let (partial_spend, private_view) = get_partial_keys(&get_camo_view_seed(master_seed), i);
        CamoKeysV1 {
            versions,
            private_spend: master_spend + partial_spend,
            private_view,
        }
    }

    fn to_view_keys(&self) -> Self::ViewKeysType {
        let spend = &self.private_spend * G;
        CamoViewKeysV1 {
            versions: self.versions,
            compressed_spend_key: spend.compress(),
            point_spend_key: spend,
            private_view: self.private_view.clone(),
        }
    }

    fn to_camo_account(&self) -> Self::AccountType {
        points_to_account(
            self.versions,
            &self.private_spend * G,
            &self.private_view * G,
        )
    }

    fn notification_key(&self) -> Key {
        Key::from_scalar(self.private_spend.clone())
    }

    fn get_versions(&self) -> CamoVersions {
        self.versions
    }

    fn receiver_ecdh(&self, sender_account: &Account) -> SecretBytes<32> {
        ecdh(&self.private_view, &sender_account.point)
    }

    fn derive_key_from_secret(&self, secret: &SecretBytes<32>, i: u32) -> Key {
        Key::from(&self.private_spend + get_account_scalar(secret, i))
    }

    fn derive_key_from_block(&self, block: &Block) -> Key {
        self.derive_key(&block.account, get_standard_index(block))
    }
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct CamoViewKeysV1 {
    versions: CamoVersions,
    compressed_spend_key: CompressedEdwardsY,
    point_spend_key: EdwardsPoint,
    private_view: Scalar,
}
impl CamoViewKeysTrait for CamoViewKeysV1 {
    type AccountType = CamoAccountV1;

    fn from_seed(
        view_seed: &SecretBytes<32>,
        master_spend: EdwardsPoint,
        i: u32,
        versions: CamoVersions,
    ) -> CamoViewKeysV1 {
        let (private_spend, private_view) = get_partial_keys(view_seed, i);
        let point_spend_key = master_spend + (private_spend * G);
        CamoViewKeysV1 {
            versions,
            compressed_spend_key: point_spend_key.compress(),
            point_spend_key,
            private_view,
        }
    }

    fn to_camo_account(&self) -> CamoAccountV1 {
        points_to_account(self.versions, self.point_spend_key, &self.private_view * G)
    }

    fn notification_account(&self) -> Account {
        Account::from_both_points(&self.point_spend_key, &self.compressed_spend_key)
    }

    fn get_versions(&self) -> CamoVersions {
        self.versions
    }

    fn receiver_ecdh(&self, sender_key: &Account) -> SecretBytes<32> {
        ecdh(&self.private_view, &sender_key.point)
    }

    fn derive_account_from_secret(&self, secret: &SecretBytes<32>, i: u32) -> Account {
        Account::from(self.point_spend_key + (get_account_scalar(secret, i) * G))
    }

    fn derive_account_from_block(&self, block: &Block) -> Account {
        self.derive_account(&block.account, get_standard_index(block))
    }
}

auto_from_impl!(From: CamoViewKeysV1 => SecretBytes<65>);
auto_from_impl!(TryFrom: SecretBytes<65> => CamoViewKeysV1);

impl From<&CamoViewKeysV1> for SecretBytes<65> {
    fn from(value: &CamoViewKeysV1) -> Self {
        let bytes: [u8; 65] = [
            [value.versions.encode_to_bits()].as_slice(),
            value.compressed_spend_key.as_bytes(),
            value.private_view.as_bytes(),
        ]
        .concat()
        .try_into()
        .unwrap();
        SecretBytes::from(bytes)
    }
}
impl TryFrom<&SecretBytes<65>> for CamoViewKeysV1 {
    type Error = NanoError;

    fn try_from(value: &SecretBytes<65>) -> Result<Self, NanoError> {
        let bytes = value.as_ref();

        let versions = CamoVersions::decode_from_bits(bytes[0]);
        let compressed_spend_key = try_compressed_from_slice(&bytes[1..33])?;
        let point_spend_key = try_point_from_slice(&bytes[1..33])?;
        let private_view = Scalar::from_canonical_bytes(bytes[33..].as_ref().try_into().unwrap())?;

        Ok(CamoViewKeysV1 {
            versions,
            compressed_spend_key,
            point_spend_key,
            private_view,
        })
    }
}

#[cfg(feature = "serde")]
impl Serialize for CamoViewKeysV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CamoViewKeysV1Serde {
            versions: self.versions,
            point_spend_key: self.point_spend_key,
            private_view: self.private_view.clone(),
        }
        .serialize(serializer)
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for CamoViewKeysV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let keys = CamoViewKeysV1Serde::deserialize(deserializer)?;
        Ok(CamoViewKeysV1 {
            versions: keys.versions,
            compressed_spend_key: keys.point_spend_key.compress(),
            point_spend_key: keys.point_spend_key,
            private_view: keys.private_view.clone(),
        })
    }
}
#[cfg(feature = "serde")]
#[derive(Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
struct CamoViewKeysV1Serde {
    versions: CamoVersions,
    point_spend_key: EdwardsPoint,
    private_view: Scalar,
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct CamoAccountV1 {
    account: String,
    versions: CamoVersions,
    compressed_spend_key: CompressedEdwardsY,
    compressed_view_key: CompressedEdwardsY,
    point_spend_key: EdwardsPoint,
    point_view_key: EdwardsPoint,
}
impl CamoAccountTrait for CamoAccountV1 {
    type KeysType = CamoKeysV1;

    fn from_keys(keys: Self::KeysType) -> CamoAccountV1 {
        keys.to_camo_account()
    }

    fn from_data(account: &str, data: &[u8]) -> Result<CamoAccountV1, NanoError> {
        account_from_data(account, data)
    }

    fn notification_account(&self) -> Account {
        Account::from_both_points(&self.point_spend_key, &self.compressed_spend_key)
    }

    fn get_versions(&self) -> CamoVersions {
        self.versions
    }

    fn sender_ecdh(&self, sender_key: &Key) -> SecretBytes<32> {
        ecdh(sender_key.as_scalar(), &self.point_view_key)
    }

    fn derive_account_from_secret(&self, secret: &SecretBytes<32>, i: u32) -> Account {
        Account::from(self.point_spend_key + (get_account_scalar(secret, i) * G))
    }

    fn derive_account_from_block(&self, block: &Block, key: &Key) -> Account {
        self.derive_account(key, get_standard_index(block))
    }
}
impl Display for CamoAccountV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.account)
    }
}

#[cfg(feature = "serde")]
impl Serialize for CamoAccountV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CamoAccountV1Serde {
            versions: self.versions,
            point_spend_key: self.point_spend_key,
            point_view_key: self.point_view_key,
        }
        .serialize(serializer)
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for CamoAccountV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let keys = CamoAccountV1Serde::deserialize(deserializer)?;
        Ok(points_to_account(
            keys.versions,
            keys.point_spend_key,
            keys.point_view_key,
        ))
    }
}
#[cfg(feature = "serde")]
#[derive(Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
struct CamoAccountV1Serde {
    versions: CamoVersions,
    point_spend_key: EdwardsPoint,
    point_view_key: EdwardsPoint,
}

camo_address_tests!(
    CamoKeysV1, CamoViewKeysV1, CamoAccountV1,
    versions!(1),
    "camo_18wydi3gmaw4aefwhkijrjw4qd87i4tc85wbnij95gz4em3qssickhpoj9i4t6taqk46wdnie7aj8ijrjhtcdgsp3c1oqnahct3otygxx4k7f3o4"
);