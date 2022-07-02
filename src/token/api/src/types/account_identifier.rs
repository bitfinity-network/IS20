use std::fmt::{Display, Formatter};
use std::str::FromStr;

use candid::{CandidType, Principal};
use serde::de::Error;
use serde::{de, Deserialize, Serialize};
use sha2::{Digest, Sha224};

pub static SUB_ACCOUNT_ZERO: Subaccount = Subaccount([0; 32]);
static ACCOUNT_DOMAIN_SEPERATOR: &[u8] = b"\x0Aaccount-id";

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct AccountIdentifier {
    pub hash: [u8; 28],
}

impl AccountIdentifier {
    pub fn new(account: Principal, sub_account: Option<Subaccount>) -> Self {
        let mut hash = Sha224::new();
        hash.update(ACCOUNT_DOMAIN_SEPERATOR);
        hash.update(account.as_slice());

        let sub_account = sub_account.unwrap_or(SUB_ACCOUNT_ZERO);
        hash.update(&sub_account.0[..]);
        Self {
            hash: hash.finalize().into(),
        }
    }

    /// Generates anonymous account identifier.
    pub fn anonymous() -> AccountIdentifier {
        Self::new(Principal::anonymous(), None)
    }

    pub fn empty() -> AccountIdentifier {
        AccountIdentifier { hash: [0; 28] }
    }

    pub fn from_hex(hex_str: &str) -> Result<AccountIdentifier, String> {
        let hex: Vec<u8> = hex::decode(hex_str).map_err(|e| e.to_string())?;
        Self::from_slice(&hex[..]).map_err(|err| match err {
            // Since the input was provided in hex, return an error that is hex-friendly.
            AccountIdParseError::InvalidLength(_) => format!(
                "{} has a length of {} but we expected a length of 64 or 56",
                hex_str,
                hex_str.len()
            ),
            AccountIdParseError::InvalidChecksum(err) => err.to_string(),
        })
    }

    /// Goes from the canonical format (with checksum) encoded in bytes rather
    /// than hex to AccountIdentifier
    pub fn from_slice(v: &[u8]) -> Result<AccountIdentifier, AccountIdParseError> {
        // Try parsing it as a 32-byte blob.
        match v.try_into() {
            Ok(h) => {
                // It's a 32-byte blob. Validate the checksum.
                check_sum(h).map_err(AccountIdParseError::InvalidChecksum)
            }
            Err(_) => {
                // Try parsing it as a 28-byte hash.
                match v.try_into() {
                    Ok(hash) => Ok(AccountIdentifier { hash }),
                    Err(_) => Err(AccountIdParseError::InvalidLength(v.to_vec())),
                }
            }
        }
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.to_vec())
    }

    pub fn to_vec(&self) -> Vec<u8> {
        [&self.generate_checksum()[..], &self.hash[..]].concat()
    }

    pub fn generate_checksum(&self) -> [u8; 4] {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&self.hash);
        hasher.finalize().to_be_bytes()
    }
}

impl Display for AccountIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.to_hex().fmt(f)
    }
}

impl FromStr for AccountIdentifier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        AccountIdentifier::from_hex(s)
    }
}

impl Serialize for AccountIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_hex().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AccountIdentifier {
    // This is the canonical way to read a this from string
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
        D::Error: de::Error,
    {
        let hex: [u8; 32] = hex::serde::deserialize(deserializer)?;
        check_sum(hex).map_err(D::Error::custom)
    }
}

impl From<Principal> for AccountIdentifier {
    fn from(pid: Principal) -> Self {
        AccountIdentifier::new(pid, None)
    }
}

impl CandidType for AccountIdentifier {
    // The type expected for account identifier is
    fn _ty() -> candid::types::Type {
        String::_ty()
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        self.to_hex().idl_serialize(serializer)
    }
}

fn check_sum(hex: [u8; 32]) -> Result<AccountIdentifier, ChecksumError> {
    // Get the checksum provided
    let found_checksum = &hex[0..4];

    // Copy the hash into a new array
    let mut hash = [0; 28];
    hash.copy_from_slice(&hex[4..32]);

    let account_id = AccountIdentifier { hash };
    let expected_checksum = account_id.generate_checksum();

    // Check the generated checksum matches
    if expected_checksum == found_checksum {
        Ok(account_id)
    } else {
        Err(ChecksumError {
            input: hex,
            expected_checksum,
            found_checksum: found_checksum.try_into().unwrap(),
        })
    }
}

/// An error reporting for invalid checksum
#[derive(Debug, PartialEq, Eq)]
pub struct ChecksumError {
    input: [u8; 32],
    expected_checksum: [u8; 4],
    found_checksum: [u8; 4],
}

impl Display for ChecksumError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Checksum failed for {}, expected check bytes {} but found {}",
            hex::encode(&self.input[..]),
            hex::encode(self.expected_checksum),
            hex::encode(self.found_checksum),
        )
    }
}

/// Error enum for reporting invalid account identifiers
#[derive(Debug, PartialEq, Eq)]
pub enum AccountIdParseError {
    InvalidLength(Vec<u8>),
    InvalidChecksum(ChecksumError),
}

impl Display for AccountIdParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChecksum(err) => write!(f, "{}", err),
            Self::InvalidLength(v) => write!(
                f,
                "Received an invalid AccountIdentifier with length \
            {} bytes instead of the expected 28 or 32",
                v.len()
            ),
        }
    }
}

/// Subaccounts are arbitrary 32-byte values
#[derive(Serialize, Deserialize, CandidType, Clone, Hash, Debug, PartialEq, Eq, Copy)]
pub struct Subaccount(pub [u8; 32]);

impl Subaccount {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl Display for Subaccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        hex::encode(self.0).fmt(f)
    }
}

// test
#[cfg(test)]
mod tests {
    use super::*;

    // test empty
    #[test]
    fn test_zero() {
        let zero = AccountIdentifier::empty();
        let zero_account_id = "807077e900000000000000000000000000000000000000000000000000000000";
        assert_eq!(zero_account_id, zero.to_hex().as_str());
    }

    #[test]
    fn check_round_trip() {
        let ai = AccountIdentifier { hash: [7; 28] };
        let res = ai.to_hex();
        assert_eq!(
            res.parse(),
            Ok(ai),
            "The account identifier doesn't change after going back and forth between a string"
        )
    }

    #[test]
    fn check_encoding() {
        let ai = AccountIdentifier { hash: [7; 28] };

        let en1 = candid::encode_one(ai).unwrap();
        let en2 = candid::encode_one(ai.to_string()).unwrap();

        assert_eq!(&en1, &en2);

        let de1: String = candid::decode_one(&en1[..]).unwrap();
        let de2: AccountIdentifier = candid::decode_one(&en2[..]).unwrap();

        assert_eq!(de1.parse(), Ok(de2));

        assert_eq!(de2, ai, "And the value itself hasn't changed");
    }

    #[test]
    fn test_from_slice() {
        let length_29 = b"123456789_123456789_123456789".to_vec();
        assert_eq!(
            AccountIdentifier::from_slice(&length_29),
            Err(AccountIdParseError::InvalidLength(length_29))
        );
        let length_27 = b"123456789_123456789_1234567".to_vec();
        assert_eq!(
            AccountIdentifier::from_slice(&length_27),
            Err(AccountIdParseError::InvalidLength(length_27))
        );

        let length_28 = b"123456789_123456789_12345678".to_vec();
        assert_eq!(
            AccountIdentifier::from_slice(&length_28),
            Ok(AccountIdentifier {
                hash: length_28.try_into().unwrap()
            })
        );
    }

    #[test]
    fn test_from_principal_to_account_id() {
        let p: Principal = "qupnt-ohzy3-npshw-oba2m-sttkq-tyawc-vufye-u5fbz-zb6yu-conr3-tqe"
            .parse()
            .unwrap();
        let ai: AccountIdentifier = p.into();

        assert_eq!(
            ai.to_hex(),
            "908ae30a212a1a73e8be38abc0f0ed525b1624fad332fac46c6cd3286241a678"
        );
    }
}
