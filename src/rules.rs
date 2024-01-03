use std::io::{Cursor, Seek, SeekFrom, Read};
#[cfg(not(feature = "async"))]
use std::net::ToSocketAddrs;

use byteorder::{LittleEndian, ReadBytesExt};

#[cfg(feature = "async")]
use tokio::net::ToSocketAddrs;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};
use crate::{A2SClient, ReadBytes};

const RULES_REQUEST: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x56];

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum Rule {
    Regular {
        name: String,
        value: String,
    },
    Mod {
        id: u32,
        name: String,
    },
}

impl Rule {
    pub fn vec_to_bytes(rules: Vec<Self>) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend(&[0xff, 0xff, 0xff, 0xff, 0x45]);

        bytes.extend(rules.len().to_le_bytes());

        for rule in rules {
            bytes.extend(rule.to_bytes());
        }

        bytes
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        match self {
            Rule::Regular { name, value } => {
                bytes.extend(name.as_bytes());
                bytes.push(0);
                bytes.extend(value.as_bytes());
                bytes.push(0);
            },
            Rule::Mod { id, name } => {
                bytes.extend(name.as_bytes());
                bytes.push(0);
                bytes.extend(id.to_le_bytes());
            },
        }

        bytes
    }

    pub fn from_cursor(mut data: Cursor<Vec<u8>>) -> Result<Vec<Self>> {
        if data.read_u8()? != 0x45 {
            return Err(Error::InvalidResponse);
        }

        let count = data.read_u16::<LittleEndian>()?;

        let mut rules = Vec::new();
        let mut mod_bytes: Vec<u8> = Vec::new();

        let mut num_mods = 0u8;
        let mut num_mod_rules = 0u8;

        for i in 0..count {
            let name = data.read_bytes_nullterm()?;
            let value = unescape(&mut data.read_bytes_nullterm()?)?;

            if i == 0 {
                num_mods = value.get(4).unwrap_or(&0).to_owned();
                num_mod_rules = name.get(1).unwrap_or(&0).to_owned();
            }

            if
                name.len() == 2 &&
                name.get(1).is_some_and(|n| n == &num_mod_rules) &&
                name.get(0).is_some_and(|n| n <= &num_mod_rules)
            {
                mod_bytes.extend(value);
            } else {
                rules.push(Rule::Regular {
                    name: String::from_utf8_lossy(&name).to_string(),
                    value: String::from_utf8_lossy(&value).to_string(),
                });
            }
        }

        let mut mods = Cursor::new(mod_bytes);
        mods.seek(SeekFrom::Start(5))?; // Skip header

        for _ in 0..num_mods {
            mods.seek(SeekFrom::Current(5))?;
            let mod_id = mods.read_u32::<LittleEndian>()?;
            let mod_name_len = mods.read_u8()?;
            let mod_name = mods.read_bytes(mod_name_len as usize)?;
            rules.push(Rule::Mod {
                id: mod_id,
                name: String::from_utf8_lossy(&mod_name).to_string(),
            });
        }

        Ok(rules)
    }
}

fn unescape(bytes: &mut Vec<u8>) -> Result<Vec<u8>> {
    let mut tmp_bytes: Vec<u8> = Vec::new();
    Cursor::new(bytes).read_to_end(&mut tmp_bytes)?;
    let mut bytes = Vec::new();
    let mut i = 0;

    while i < tmp_bytes.len() {
        let Some(curr_byte) = tmp_bytes.get(i) else { break; };
        let is_escape_byte = curr_byte == &0x01;
        if is_escape_byte {
            let Some(next_byte) = tmp_bytes.get(i + 1) else { break; };
            match next_byte {
                0x01 => {
                    bytes.push(0x01);
                    i += 2;
                    continue;
                },
                0x02 => {
                    bytes.push(0x00);
                    i += 2;
                    continue;
                },
                0x03 => {
                    bytes.push(0xFF);
                    i += 2;
                    continue;
                },
                _ => ()
            }
        }
        bytes.push(*curr_byte);
        i += 1;
    }

    Ok(bytes)
}

impl A2SClient {
    #[cfg(feature = "async")]
    pub async fn rules<A: ToSocketAddrs>(&self, addr: A) -> Result<Vec<Rule>> {
        let data = self.do_challenge_request(addr, &RULES_REQUEST).await?;
        Rule::from_cursor(Cursor::new(data))
    }

    #[cfg(not(feature = "async"))]
    pub fn rules<A: ToSocketAddrs>(&self, addr: A) -> Result<Vec<Rule>> {
        let data = self.do_challenge_request(addr, &RULES_REQUEST)?;
        Rule::from_cursor(Cursor::new(data))
    }
}
