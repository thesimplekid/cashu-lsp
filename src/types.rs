use std::str::FromStr;

use ldk_node::UserChannelId;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning::ln::msgs::SocketAddress;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

// Custom serialization for UserChannelId
mod user_channel_id_serde {
    use super::*;
    use ldk_node::UserChannelId;

    pub fn serialize<S>(
        channel_id: &Option<UserChannelId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match channel_id {
            Some(id) => {
                // Convert the internal u128 to a string
                let value = id.0.to_string();
                serializer.serialize_str(&value)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<UserChannelId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionalUserChannelIdVisitor;

        impl<'de> Visitor<'de> for OptionalUserChannelIdVisitor {
            type Value = Option<UserChannelId>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing a u128 channel ID or null")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let id = value.parse::<u128>().map_err(E::custom)?;
                Ok(Some(UserChannelId(id)))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }
        }

        deserializer.deserialize_option(OptionalUserChannelIdVisitor)
    }
}

// Custom serialization for SocketAddress
mod socket_address_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(address: &SocketAddress, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&address.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SocketAddress, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SocketAddressVisitor;

        impl<'de> Visitor<'de> for SocketAddressVisitor {
            type Value = SocketAddress;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing a socket address")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                SocketAddress::from_str(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(SocketAddressVisitor)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct QuoteInfo {
    pub id: Uuid,
    pub channel_size_sats: u64,
    pub push_amount_sats: Option<u64>,
    pub expected_payment_sats: u64,
    pub node_pubkey: PublicKey,
    #[serde(with = "socket_address_serde")]
    pub addr: SocketAddress,
    pub state: QuoteState,
    #[serde(with = "user_channel_id_serde")]
    pub channel_id: Option<UserChannelId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelQuoteRequest {
    pub channel_size_sats: u64,
    pub node_pubkey: PublicKey,
    #[serde(with = "socket_address_serde")]
    pub addr: SocketAddress,
    pub push_amount: Option<u64>,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum QuoteState {
    Unpaid,
    Paid,
    ChannelPending,
    ChannelOpen,
    ChannelExpired,
}
