//! An Identity is a signer that also has an address on the MANY protocol.
use crate::Address;
use coset::{CoseKey, CoseSign1};
use many_error::ManyError;
use std::fmt::Formatter;
use std::ops::Deref;
use std::sync::Arc;

/// An Identity is anything that is a unique address and can sign messages.
pub trait Identity: Send + Sync {
    /// The address of the identity.
    fn address(&self) -> Address;

    /// Its public key. In some cases, the public key is absent or unknown.
    fn public_key(&self) -> Option<CoseKey>;

    /// Signs an envelope with this identity.
    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError>;
}

/// A Verifier is the other side of the signature. It verifies that an envelope
/// matches its signature, either using the envelope or the message fields.
/// It should also resolve the address used to sign or represent the signer
/// the envelope, and returns it.
pub trait Verifier: Send {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError>;
}

#[derive(Debug, Clone)]
pub struct AnonymousIdentity;

impl Identity for AnonymousIdentity {
    fn address(&self) -> Address {
        Address::anonymous()
    }

    fn public_key(&self) -> Option<CoseKey> {
        None
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        // An anonymous envelope has no signature, or special header.
        Ok(envelope)
    }
}

#[cfg(feature = "testing")]
mod testing {
    use crate::{Address, Verifier};

    /// Accept ALL envelopes, and uses the key id as is to resolve the address.
    /// No verification is made. This should NEVER BE used for production.
    pub struct AcceptAllVerifier;

    impl Verifier for AcceptAllVerifier {
        fn verify_1(&self, envelope: &coset::CoseSign1) -> Result<Address, many_error::ManyError> {
            // Does not verify the signature and key id.
            let kid = &envelope.protected.header.key_id;
            if kid.is_empty() {
                Ok(Address::anonymous())
            } else {
                Address::from_bytes(kid)
            }
        }
    }
}

#[cfg(feature = "testing")]
pub use testing::*;

// Implement Identity for everything that implements Deref<Target = Identity>.
impl<I: Identity + ?Sized + 'static, T: Deref<Target = I> + Send + Sync> Identity for T {
    fn address(&self) -> Address {
        self.deref().address()
    }

    fn public_key(&self) -> Option<CoseKey> {
        self.deref().public_key()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        self.deref().sign_1(envelope)
    }
}

// Implement Verifier for everything that implements Deref<Target = Verifier>.
impl<V: Verifier + ?Sized + 'static, T: Deref<Target = V> + Send> Verifier for T {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
        self.deref().verify_1(envelope)
    }
}

pub struct ErrorVerifier;

impl Verifier for ErrorVerifier {
    fn verify_1(&self, _envelope: &CoseSign1) -> Result<Address, ManyError> {
        Err(ManyError::could_not_verify_signature("No verifier"))
    }
}

macro_rules! declare_one_of_verifiers {
    ( $id: ident, $( $name: ident: $index: tt ),* ) => {
        pub struct $id< $( $name: Verifier ),* >( $( $name ),* );
        impl< $( $name: Verifier ),* > Verifier for $id<$( $name ),*> {
            fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
                let mut errs = Vec::new();
                $(
                    match self. $index . verify_1(envelope) {
                        Ok(a) => return Ok(a),
                        Err(e) => errs.push(e.to_string()),
                    }
                )*

                Err(ManyError::could_not_verify_signature(errs.join(", ")))
            }
        }
        impl< $( $name: Verifier ),* > From<( $( $name, )* )> for $id<$( $name ),*> {
            fn from( v: ( $( $name, )* )) -> Self {
                Self($( v. $index ),*)
            }
        }
        impl< $( $name: Verifier + 'static + Send + Sync ),* > Into<OneOfVerifier> for $id<$( $name ),*> {
            fn into( self ) -> OneOfVerifier {
                OneOfVerifier(Arc::new(self))
            }
        }
    }
}

#[derive(Clone)]
pub struct OneOfVerifier(Arc<dyn Verifier + Send + Sync>);

impl std::fmt::Debug for OneOfVerifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("OneOfVerifier").finish()
    }
}

impl Verifier for OneOfVerifier {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
        self.0.verify_1(envelope)
    }
}

#[macro_export]
macro_rules! one_of_verifier {
    () => {
        $crate::ErrorVerifier()
    };
    ( $a: expr $(,)? ) => {
        $crate::OneOfVerifier1::from(($a,))
    };
    ( $a: expr, $b: expr $(,)? ) => {
        $crate::OneOfVerifier2::from(($a, $b))
    };
    ( $a: expr, $b: expr, $c: expr $(,)? ) => {
        $crate::OneOfVerifier3::from(($a, $b, $c))
    };
    ( $a: expr, $b: expr, $c: expr, $d: expr $(,)? ) => {
        $crate::OneOfVerifier4::from(($a, $b, $c, $d))
    };
}

// 8 outta be enough for everyone (but you can also ((a, b), (c, d), ...) recursively).
declare_one_of_verifiers!(OneOfVerifier1, A: 0);
declare_one_of_verifiers!(OneOfVerifier2, A: 0, B: 1);
declare_one_of_verifiers!(OneOfVerifier3, A: 0, B: 1, C: 2);
declare_one_of_verifiers!(OneOfVerifier4, A: 0, B: 1, C: 2, D: 3);
declare_one_of_verifiers!(OneOfVerifier5, A: 0, B: 1, C: 2, D: 3, E: 4);
declare_one_of_verifiers!(OneOfVerifier6, A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
declare_one_of_verifiers!(OneOfVerifier7, A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
declare_one_of_verifiers!(OneOfVerifier8, A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);

pub mod verifiers {
    use crate::{Address, Verifier};
    use coset::CoseSign1;
    use many_error::ManyError;
    use tracing::trace;

    #[derive(Clone, Debug)]
    pub struct AnonymousVerifier;

    impl Verifier for AnonymousVerifier {
        fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
            let kid = &envelope.protected.header.key_id;
            if !kid.is_empty() {
                if Address::from_bytes(kid)?.is_anonymous() {
                    trace!("Anonymous message");
                    Ok(Address::anonymous())
                } else {
                    Err(ManyError::unknown("Anonymous requires no key id."))
                }
            } else if !envelope.signature.is_empty() {
                Err(ManyError::unknown("Anonymous requires no signature."))
            } else {
                trace!("Anonymous message");
                Ok(Address::anonymous())
            }
        }
    }
}
