use std::borrow::Cow;
use serde::{Serialize, Deserialize};
use rlp::RlpStream;
use tiny_keccak::keccak256 as keccak;
use impl_serde::serialize as bytes;

pub use ethereum_types::{Address, U256};

/// Hex-serialized shim for `Vec<u8>`.
#[derive(Serialize, Deserialize, Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Clone)]
pub struct Bytes(#[serde(with="bytes")] pub Vec<u8>);
impl From<Vec<u8>> for Bytes {
	fn from(s: Vec<u8>) -> Self { Bytes(s) }
}

impl std::ops::Deref for Bytes {
	type Target = [u8];
	fn deref(&self) -> &[u8] { &self.0[..] }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all="camelCase")]
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
#[serde(rename_all="camelCase")]
pub struct SignTransaction<'a> {
    pub transaction: Cow<'a, Transaction>,
    pub chain_id: u64,
}

impl<'a> SignTransaction<'a> {
    pub fn hash(&self) -> [u8; 32] {
        SignedTransaction {
            transaction: Cow::Borrowed(&*self.transaction),
            v: self.chain_id,
            r: 0.into(),
            s: 0.into()
        }.hash()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
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
				nonce: d.val_at(0)?,
				gas_price: d.val_at(1)?,
				gas: d.val_at(2)?,
				to: d.val_at(3)?,
                from: Default::default(),
				value: d.val_at(4)?,
				data: d.val_at::<Vec<u8>>(5)?.into(),
			}),
			v: d.val_at(6)? ,
			r: d.val_at(7)?,
			s: d.val_at(8)?,
		})
	}
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
            transaction: transaction.into(),
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
			_ => 4
		}
	}

    pub fn chain_id(&self) -> Option<u64> {
        replay_protection::chain_id(self.v)
    }

    pub fn hash(&self) -> [u8; 32] {
        self.with_rlp(|s| keccak(s.as_raw()))
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
			_ => None
		}
	}
}
