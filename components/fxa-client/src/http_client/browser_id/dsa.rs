/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use super::BrowserIDKeyPair;
use crate::errors::*;
use openssl::{
    bn::BigNum,
    hash::MessageDigest,
    pkey::{PKey, Private},
    dsa::Dsa,
    sign::Signer,
};
use serde::{
    de::{self, Deserialize, Deserializer, MapAccess, Visitor},
    ser::{self, Serialize, SerializeStruct, Serializer},
};
use serde_json::{self, json};
use std::fmt;

pub struct DSABrowserIDKeyPair {
    key: PKey<Private>,
}

impl DSABrowserIDKeyPair {
    fn from_dsa(dsa: Dsa<Private>) -> Result<DSABrowserIDKeyPair> {
        let key = PKey::from_dsa(dsa)?;
        Ok(DSABrowserIDKeyPair { key })
    }

    pub fn generate_random(len: u32) -> Result<DSABrowserIDKeyPair> {
        let dsa = Dsa::generate(len)?;
        DSABrowserIDKeyPair::from_dsa(dsa)
    }

    #[allow(dead_code)]
    pub fn from_exponents_base10(g: &str, p: &str, q: &str, x: &str, y: &str) -> Result<DSABrowserIDKeyPair> {
        let g = BigNum::from_dec_str(g)?;
        let p = BigNum::from_dec_str(p)?;
        let q = BigNum::from_dec_str(q)?;
        let x = BigNum::from_dec_str(x)?;
        let y = BigNum::from_dec_str(y)?;
        let dsa = Dsa::from_private_components(g, p, q, x, y)?;
        DSABrowserIDKeyPair::from_dsa(dsa)
    }
}

impl BrowserIDKeyPair for DSABrowserIDKeyPair {
    fn get_algo(&self) -> String {
        format!("DS{}", self.key.bits() / 8)
    }

    fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        let mut signer = Signer::new(MessageDigest::sha256(), &self.key)?;
        signer.update(message)?;
        signer.sign_to_vec().map_err(|e| e.into())
    }

    fn verify_message(&self, message: &[u8], signature: &[u8]) -> Result<bool> {
        unimplemented!("TODO")
    }

    fn to_json(&self, include_private: bool) -> Result<serde_json::Value> {
        if include_private {
            panic!("Not implemented!");
        }
        let dsa = self.key.dsa()?;
        let y = format!("{}", dsa.pub_key().to_dec_str()?);
        let g = format!("{}", dsa.g().to_dec_str()?);
        let p = format!("{}", dsa.p().to_dec_str()?);
        let q = format!("{}", dsa.q().to_dec_str()?);
        Ok(json!({
          "algorithm": "DS",
          "y": y,
          "g": g,
          "p": p,
          "q": q
        }))
    }
}

impl Clone for DSABrowserIDKeyPair {
    fn clone(&self) -> DSABrowserIDKeyPair {
        let dsa = self.key.dsa().unwrap().clone();
        DSABrowserIDKeyPair::from_dsa(dsa).unwrap() // Yuck
    }
}

impl fmt::Debug for DSABrowserIDKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<dsa_key_pair>")
    }
}

impl Serialize for DSABrowserIDKeyPair {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DSABrowserIDKeyPair", 2)?;
        let dsa = self
            .key
            .dsa()
            .map_err(|err| ser::Error::custom(err.to_string()))?;
        let g = dsa
            .g()
            .to_dec_str()
            .map_err(|err| ser::Error::custom(err.to_string()))?;
        let p = dsa
            .p()
            .to_dec_str()
            .map_err(|err| ser::Error::custom(err.to_string()))?;
        let q = dsa
            .q()
            .to_dec_str()
            .map_err(|err| ser::Error::custom(err.to_string()))?;
        let x = dsa
            .priv_key()
            .to_dec_str()
            .map_err(|err| ser::Error::custom(err.to_string()))?;
        let y = dsa
            .pub_key()
            .to_dec_str()
            .map_err(|err| ser::Error::custom(err.to_string()))?;
        state.serialize_field("g", &format!("{}", g))?;
        state.serialize_field("p", &format!("{}", p))?;
        state.serialize_field("q", &format!("{}", q))?;
        state.serialize_field("x", &format!("{}", x))?;
        state.serialize_field("y", &format!("{}", y))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for DSABrowserIDKeyPair {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            G,
            P,
            Q,
            X,
            Y,
        };

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`g`, `p`, `q`, `x`, `y`")
                    }

                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "g" => Ok(Field::G),
                            "p" => Ok(Field::P),
                            "q" => Ok(Field::Q),
                            "x" => Ok(Field::X),
                            "y" => Ok(Field::Y),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct DSABrowserIDKeyPairVisitor;

        impl<'de> Visitor<'de> for DSABrowserIDKeyPairVisitor {
            type Value = DSABrowserIDKeyPair;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct DSABrowserIDKeyPair")
            }

            fn visit_map<V>(self, mut map: V) -> std::result::Result<DSABrowserIDKeyPair, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut g = None;
                let mut p = None;
                let mut q = None;
                let mut x = None;
                let mut y = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::G => {
                            if g.is_some() {
                                return Err(de::Error::duplicate_field("g"));
                            }
                            g = Some(map.next_value()?);
                        }
                        Field::P => {
                            if p.is_some() {
                                return Err(de::Error::duplicate_field("p"));
                            }
                            p = Some(map.next_value()?);
                        }
                        Field::Q => {
                            if q.is_some() {
                                return Err(de::Error::duplicate_field("q"));
                            }
                            q = Some(map.next_value()?);
                        }
                        Field::X => {
                            if x.is_some() {
                                return Err(de::Error::duplicate_field("x"));
                            }
                            x = Some(map.next_value()?);
                        }
                        Field::Y => {
                            if y.is_some() {
                                return Err(de::Error::duplicate_field("y"));
                            }
                            y = Some(map.next_value()?);
                        }
                    }
                }
                let g = g.ok_or_else(|| de::Error::missing_field("g"))?;
                let g =
                    BigNum::from_dec_str(g).map_err(|err| de::Error::custom(err.to_string()))?;
                let p = p.ok_or_else(|| de::Error::missing_field("p"))?;
                let p =
                    BigNum::from_dec_str(p).map_err(|err| de::Error::custom(err.to_string()))?;
                let q = q.ok_or_else(|| de::Error::missing_field("q"))?;
                let q =
                    BigNum::from_dec_str(q).map_err(|err| de::Error::custom(err.to_string()))?;
                let x = x.ok_or_else(|| de::Error::missing_field("x"))?;
                let x =
                    BigNum::from_dec_str(x).map_err(|err| de::Error::custom(err.to_string()))?;
                let y = y.ok_or_else(|| de::Error::missing_field("y"))?;
                let y =
                    BigNum::from_dec_str(y).map_err(|err| de::Error::custom(err.to_string()))?;
                let dsa = Dsa::from_private_components(g, p, q, x, y).map_err(|err| de::Error::custom(err.to_string()))?;
                DSABrowserIDKeyPair::from_dsa(dsa).map_err(|err| de::Error::custom(err.to_string()))
            }
        }

        const FIELDS: &'static [&'static str] = &["g", "p", "q", "x", "y"];
        deserializer.deserialize_struct("DSABrowserIDKeyPair", FIELDS, DSABrowserIDKeyPairVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let key_pair = DSABrowserIDKeyPair::generate_random(2048).unwrap();
        let as_json = serde_json::to_string(&key_pair).unwrap();
        let _key_pair: DSABrowserIDKeyPair = serde_json::from_str(&as_json).unwrap();
    }
}
