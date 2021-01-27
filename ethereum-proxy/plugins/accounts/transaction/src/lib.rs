// Copyright (c) 2018-2020 jsonrpc-proxy contributors.
//
// This file is part of jsonrpc-proxy
// (see https://github.com/tomusdrw/jsonrpc-proxy).
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.
//! A set of primitives to construct ethereum transactions.

use impl_serde::serialize as bytes;
use rlp::RlpStream;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use tiny_keccak::keccak256 as keccak;

pub use ethereum_types::{Address, U256};

/// Hex-serialized shim for `Vec<u8>`.
#[derive(Serialize, Deserialize, Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Clone, Default)]
pub struct Bytes(#[serde(with = "bytes")] pub Vec<u8>);
impl From<Vec<u8>> for Bytes {
    fn from(s: Vec<u8>) -> Self {
        Bytes(s)
    }
}

impl std::ops::Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas: U256,
    pub gas_price: U256,
    pub value: U256,
    pub data: Bytes,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SignTransaction<'a> {
    pub transaction: Cow<'a, Transaction>,
    pub chain_id: u64,
}

impl<'a> SignTransaction<'a> {
    pub fn owned(tx: Transaction, chain_id: u64) -> Self {
        Self {
            transaction: Cow::Owned(tx),
            chain_id,
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        SignedTransaction {
            transaction: Cow::Borrowed(&*self.transaction),
            v: self.chain_id,
            r: 0.into(),
            s: 0.into(),
        }
        .hash()
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SignedTransaction<'a> {
    pub transaction: Cow<'a, Transaction>,
    pub v: u64,
    pub r: U256,
    pub s: U256,
}

impl<'a> rlp::Decodable for SignedTransaction<'a> {
    fn decode(d: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if d.item_count()? != 9 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        Ok(SignedTransaction {
            transaction: Cow::Owned(Transaction {
                nonce: d.val_at(0).map_err(|e| debug("nonce", e))?,
                gas_price: d.val_at(1).map_err(|e| debug("gas_price", e))?,
                gas: d.val_at(2).map_err(|e| debug("gas", e))?,
                to: {
                    let to = d.at(3).map_err(|e| debug("to", e))?;
                    if to.is_empty() {
                        if to.is_data() {
                            None
                        } else {
                            return Err(rlp::DecoderError::RlpExpectedToBeData);
                        }
                    } else {
                        Some(to.as_val().map_err(|e| debug("to", e))?)
                    }
                },
                from: Default::default(),
                value: d.val_at(4).map_err(|e| debug("value", e))?,
                data: d.val_at::<Vec<u8>>(5).map_err(|e| debug("data", e))?.into(),
            }),
            v: d.val_at(6).map_err(|e| debug("v", e))?,
            r: d.val_at(7).map_err(|e| debug("r", e))?,
            s: d.val_at(8).map_err(|e| debug("s", e))?,
        })
    }
}

fn debug(s: &str, err: rlp::DecoderError) -> rlp::DecoderError {
    log::error!("Error decoding field: {}: {:?}", s, err);
    err
}

impl<'a> rlp::Encodable for SignedTransaction<'a> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(9);
        s.append(&self.transaction.nonce);
        s.append(&self.transaction.gas_price);
        s.append(&self.transaction.gas);
        match self.transaction.to.as_ref() {
            None => s.append(&""),
            Some(addr) => s.append(addr),
        };
        s.append(&self.transaction.value);
        s.append(&self.transaction.data.0);
        s.append(&self.v);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl<'a> SignedTransaction<'a> {
    pub fn new(
        transaction: Cow<'a, Transaction>,
        chain_id: u64,
        v: u8,
        r: [u8; 32],
        s: [u8; 32],
    ) -> Self {
        let v = replay_protection::add(v, chain_id);
        let r = U256::from_big_endian(&r);
        let s = U256::from_big_endian(&s);

        Self {
            transaction,
            v,
            r,
            s,
        }
    }

    pub fn standard_v(&self) -> u8 {
        match self.v {
            v if v == 27 => 0,
            v if v == 28 => 1,
            v if v >= 35 => ((v - 1) % 2) as u8,
            _ => 4,
        }
    }

    pub fn chain_id(&self) -> Option<u64> {
        replay_protection::chain_id(self.v)
    }

    pub fn hash(&self) -> [u8; 32] {
        self.with_rlp(|s| keccak(s.as_raw()))
    }

    pub fn bare_hash(&self) -> [u8; 32] {
        let chain_id = self.chain_id().unwrap_or_default();

        SignTransaction {
            transaction: std::borrow::Cow::Borrowed(&self.transaction),
            chain_id,
        }
        .hash()
    }

    pub fn to_rlp(&self) -> Vec<u8> {
        self.with_rlp(|s| s.drain())
    }

    fn with_rlp<R>(&self, f: impl FnOnce(RlpStream) -> R) -> R {
        let mut s = RlpStream::new();
        rlp::Encodable::rlp_append(self, &mut s);
        f(s)
    }
}

mod replay_protection {
    /// Adds chain id into v
    pub fn add(v: u8, chain_id: u64) -> u64 {
        v as u64 + 35 + chain_id * 2
    }

    /// Extracts chain_id from v
    pub fn chain_id(v: u64) -> Option<u64> {
        match v {
            v if v >= 35 => Some((v - 35) / 2),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_rlp_round_trip() {
        let transaction = Transaction {
            from: Default::default(),
            to: None,
            nonce: 5.into(),
            gas_price: 15.into(),
            gas: 69.into(),
            data: Default::default(),
            value: 1_000.into(),
        };
        let t = SignedTransaction::new(Cow::Owned(transaction), 105, 0, [1; 32], [1; 32]);

        let encoded = rlp::encode(&t);
        let decoded: SignedTransaction = rlp::decode(&encoded).unwrap();

        assert_eq!(t, decoded);
    }

    #[test]
    fn transaction_rlp_round_trip2() {
        let transaction = Transaction {
            from: Default::default(),
            to: Some(ethereum_types::H160::repeat_byte(5)),
            nonce: 5.into(),
            gas_price: 15.into(),
            gas: 69.into(),
            data: Default::default(),
            value: 1_000.into(),
        };
        let t = SignedTransaction::new(Cow::Owned(transaction), 105, 0, [1; 32], [1; 32]);

        let encoded = rlp::encode(&t);
        let decoded: SignedTransaction = rlp::decode(&encoded).unwrap();

        assert_eq!(t, decoded);
    }
}
