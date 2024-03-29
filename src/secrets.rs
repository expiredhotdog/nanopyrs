use crate::auto_from_impl;
use auto_ops::{impl_op_ex, impl_op_ex_commutative};
use curve25519_dalek::{
    edwards::EdwardsPoint,
    scalar::{clamp_integer, Scalar as RawScalar},
};
use std::convert::From;
use std::fmt::Debug;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::error::NanoError;

/// Create a `SecretBytes<N>`
#[macro_export]
macro_rules! secret {
    ($data: expr) => {{
        use $crate::SecretBytes;
        SecretBytes::from($data)
    }};
}
/// Create a `Scalar`
#[macro_export]
macro_rules! scalar {
    ($data: expr) => {{
        use $crate::Scalar;
        Scalar::from($data)
    }};
}

/// A wrapper for `[u8; N]` that automatically calls `zeroize` when dropped
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct SecretBytes<const N: usize> {
    bytes: Box<[u8; N]>,
}
impl<const N: usize> SecretBytes<N> {
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.bytes
    }
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}
impl<const N: usize> From<[u8; N]> for SecretBytes<N> {
    fn from(mut value: [u8; N]) -> Self {
        let secret = SecretBytes {
            bytes: Box::new(value),
        };
        value.zeroize();
        secret
    }
}
impl<const N: usize> From<SecretBytes<N>> for [u8; N] {
    fn from(value: SecretBytes<N>) -> Self {
        *value.bytes
    }
}
impl<const N: usize> AsMut<[u8; N]> for SecretBytes<N> {
    fn as_mut(&mut self) -> &mut [u8; N] {
        self.bytes.as_mut()
    }
}
impl<const N: usize> AsRef<[u8; N]> for SecretBytes<N> {
    fn as_ref(&self) -> &[u8; N] {
        &self.bytes
    }
}
impl<const N: usize> Debug for SecretBytes<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[secret value]")
    }
}
#[cfg(feature = "serde")]
impl<const N: usize> Serialize for SecretBytes<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SecretBytesSerde {
            bytes: *self.as_bytes(),
        }
        .serialize(serializer)
    }
}
#[cfg(feature = "serde")]
impl<'de, const N: usize> Deserialize<'de> for SecretBytes<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(SecretBytes::from(
            SecretBytesSerde::deserialize(deserializer)?.bytes,
        ))
    }
}
/// Serde-compatible representation of `SecretBytes`
#[cfg(feature = "serde")]
#[derive(Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
struct SecretBytesSerde<const N: usize> {
    #[serde(with = "serde_arrays")]
    bytes: [u8; N],
}

/// A wrapper for `curve25519_dalek::scalar::Scalar` that automatically calls `zeroize` when dropped
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Scalar(Box<RawScalar>);
impl Scalar {
    /// From 32 bytes, manipulating them as needed
    pub fn from_bytes_mod_order(mut bytes: [u8; 32]) -> Scalar {
        let raw = RawScalar::from_bytes_mod_order(bytes);
        bytes.zeroize();
        Scalar::from(raw)
    }
    /// From 64 bytes, manipulating them as needed
    pub fn from_bytes_mod_order_wide(mut bytes: [u8; 64]) -> Scalar {
        let raw = RawScalar::from_bytes_mod_order_wide(&bytes);
        bytes.zeroize();
        Scalar::from(raw)
    }
    /// From 32 bytes, keeping them exactly the same
    pub fn from_canonical_bytes(mut bytes: [u8; 32]) -> Result<Scalar, NanoError> {
        let raw = RawScalar::from_canonical_bytes(bytes);
        if raw.is_none().into() {
            return Err(NanoError::InvalidCurvePoint);
        }
        bytes.zeroize();
        Ok(Scalar::from(raw.unwrap()))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.as_ref().as_bytes()
    }
    pub fn as_slice(&self) -> &[u8] {
        self.as_bytes().as_slice()
    }
}

auto_from_impl!(From: SecretBytes<32> => Scalar);
auto_from_impl!(From: SecretBytes<64> => Scalar);

impl From<&SecretBytes<32>> for Scalar {
    fn from(value: &SecretBytes<32>) -> Self {
        Scalar(Box::new(RawScalar::from_bytes_mod_order(clamp_integer(
            *value.as_ref(),
        ))))
    }
}
impl From<&SecretBytes<64>> for Scalar {
    fn from(value: &SecretBytes<64>) -> Self {
        Scalar::from(RawScalar::from_bytes_mod_order_wide(value.as_ref()))
    }
}
impl From<[u8; 32]> for Scalar {
    fn from(value: [u8; 32]) -> Self {
        Scalar::from(secret!(value))
    }
}
impl From<[u8; 64]> for Scalar {
    fn from(value: [u8; 64]) -> Self {
        Scalar::from(secret!(value))
    }
}
impl From<RawScalar> for Scalar {
    fn from(mut value: RawScalar) -> Self {
        let scalar = Scalar(Box::new(value));
        value.zeroize();
        scalar
    }
}
impl From<Scalar> for RawScalar {
    fn from(value: Scalar) -> Self {
        *value.as_ref()
    }
}
impl AsRef<RawScalar> for Scalar {
    fn as_ref(&self) -> &RawScalar {
        &self.0
    }
}
impl Debug for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[secret value]")
    }
}

impl_op_ex!(-|a: &Scalar| -> Scalar { Scalar::from(-a.as_ref()) });

impl_op_ex!(+ |a: &Scalar, b: &Scalar| -> Scalar {
    Scalar::from(a.as_ref() + b.as_ref())
});
impl_op_ex!(*|a: &Scalar, b: &Scalar| -> Scalar { Scalar::from(a.as_ref() * b.as_ref()) });
impl_op_ex!(-|a: &Scalar, b: &Scalar| -> Scalar { Scalar::from(a.as_ref() - b.as_ref()) });

impl_op_ex_commutative!(+ |a: &Scalar, b: &RawScalar| -> Scalar {
    Scalar::from(a.as_ref() + b)
});
impl_op_ex_commutative!(*|a: &Scalar, b: &RawScalar| -> Scalar { Scalar::from(a.as_ref() * b) });
impl_op_ex!(-|a: &Scalar, b: &RawScalar| -> Scalar { Scalar::from(a.as_ref() - b) });
impl_op_ex!(-|a: &RawScalar, b: &Scalar| -> Scalar { Scalar::from(a - b.as_ref()) });

impl_op_ex_commutative!(*|a: &Scalar, b: &EdwardsPoint| -> EdwardsPoint { a.as_ref() * b });

#[cfg(test)]
#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;
    use crate::serde_test;

    serde_test!(secret_bytes: SecretBytes::from([99; 32]) => 32);
    serde_test!(scalar: Scalar::from_bytes_mod_order([99; 32]) => 32);
}
