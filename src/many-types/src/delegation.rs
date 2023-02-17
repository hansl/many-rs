use crate::{Memo, Timestamp, VecOrSingle};
use coset::{CoseSign1, CoseSign1Builder};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use minicbor::{Decode, Encode};

/// A delegation certificate.
#[derive(Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct Certificate {
    /// The address of the delegated identity (`Alice` in the example).
    #[n(0)]
    pub from: Address,

    /// The address of the identity that can use the above identity (`Bob` in the example).
    /// If this contains multiple addresses, all of them can be used to delegate.
    /// The threshold field can restrict this behaviour.
    #[n(1)]
    pub to: VecOrSingle<Address>,

    /// An expiration timestamp. If the system time is past this timestamp, the certificate is
    /// invalid and the server MUST return an error without opening the envelope further.
    #[n(2)]
    pub expiration: Timestamp,

    #[n(3)]
    pub memo: Option<Memo>,

    #[n(4)]
    pub r#final: Option<bool>,

    /// The threshold. If missing, this is equivalent to 1.
    /// If 0, this certificate is invalid.
    /// If this is greater than the `from` list, this certificate can be
    /// either considered invalid or never achievable.
    /// If threshold is set, the server is allowed to keep this in a cache
    /// temporarily for more signatures to accumulate.
    #[n(5)]
    pub threshold: Option<u64>,
}

impl Certificate {
    pub fn new(from: Address, to: impl Into<VecOrSingle<Address>>, expiration: Timestamp) -> Self {
        Self {
            from: from,
            to: to.into(),
            expiration,
            memo: None,
            r#final: None,
            threshold: None,
        }
    }

    pub fn with_threshold(self, t: u64) -> Self {
        Self {
            threshold: Some(T),
            ..self
        }
    }

    pub fn with_final(self, v: bool) -> Self {
        Self {
            r#final: if v { Some(true) } else { None },
            ..self
        }
    }

    pub fn is_final(&self) -> bool {
        self.r#final == Some(true)
    }

    pub fn sign(&self, id: &impl Identity) -> Result<CoseSign1, ManyError> {
        let address = id.address();
        if !self.from.matches(&address) {
            return Err(ManyError::unknown("From does not match identity."));
        }

        // Create the CoseSign1, then sign it.
        let cose_sign_1 = CoseSign1Builder::new()
            .payload(minicbor::to_vec(self).map_err(ManyError::deserialization_error)?)
            .build();

        id.sign_1(cose_sign_1)
    }

    pub fn decode_and_verify(
        envelope: &CoseSign1,
        verifier: &impl Verifier,
        now: Timestamp,
        is_last: bool,
    ) -> Result<Self, ManyError> {
        let from = verifier.verify_1(envelope)?;
        let payload = envelope
            .payload
            .as_ref()
            .ok_or_else(|| ManyError::unknown("Empty envelope."))?;
        let certificate: Self =
            minicbor::decode(payload).map_err(ManyError::deserialization_error)?;

        if !certificate.from.matches(&from) {
            return Err(ManyError::unknown("From does not match identity."));
        }
        if certificate.expiration <= now {
            return Err(ManyError::unknown("Delegation certificate expired."));
        }
        if certificate.is_final() && !is_last {
            return Err(ManyError::unknown("Delegation certificate is final."));
        }

        Ok(certificate)
    }
}

#[cfg(test)]
mod tests {
    use super::Certificate;
    use crate::Timestamp;
    use coset::CoseSign1Builder;
    use many_identity::{AnonymousIdentity, Identity};
    use many_identity_dsa::ed25519::generate_random_ed25519_identity;
    use many_identity_dsa::CoseKeyVerifier;

    #[test]
    fn valid() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(id1.address(), id2.address(), now + 1000);

        let envelope = certificate.sign(&id1).unwrap();
        let result =
            Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now(), true);

        assert_eq!(result, Ok(certificate));
    }

    #[test]
    fn valid_final() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let mut certificate = Certificate::new(id1.address(), id2.address(), now + 1000);
        certificate.r#final = Some(true);

        let envelope = certificate.sign(&id1).unwrap();
        let result =
            Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now(), true);

        assert_eq!(result, Ok(certificate));
    }

    #[test]
    fn invalid_expiration() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(id1.address(), id2.address(), now);

        let envelope = certificate.sign(&id1).unwrap();
        let result =
            Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now(), true);

        assert!(result.is_err());
    }

    #[test]
    fn invalid_from_sign() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(AnonymousIdentity.address(), id2.address(), now);
        assert!(certificate.sign(&id1).is_err());
    }

    #[test]
    fn invalid_from() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(AnonymousIdentity.address(), id2.address(), now);

        // Create envelope using the wrong signing identity.
        let cose_sign_1 = CoseSign1Builder::new()
            .payload(minicbor::to_vec(certificate).unwrap())
            .build();
        let envelope = id1.sign_1(cose_sign_1).unwrap();

        let result =
            Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now(), true);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_final() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let mut certificate = Certificate::new(AnonymousIdentity.address(), id2.address(), now);
        certificate.r#final = Some(true);

        // Create envelope using the wrong signing identity.
        let cose_sign_1 = CoseSign1Builder::new()
            .payload(minicbor::to_vec(certificate).unwrap())
            .build();
        let envelope = id1.sign_1(cose_sign_1).unwrap();

        let result =
            Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now(), false);
        assert!(result.is_err());
    }
}
