use std::cmp::{Eq, PartialEq};
use std::hash::{Hash, Hasher};

use serenity::model::id::UserId;

use regex::Regex;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error;

/// Wrapper around a `Regex, UserId` tuple that implements `PartialEq`, `Eq` and `Hash` manually,
/// using the `Regex::as_str()` function as the `Regex` object itself cannot be hashed.
pub struct Grep(pub Regex, pub UserId);

impl PartialEq<Grep> for Grep {
    fn eq(&self, other: &Self) -> bool {
        let Grep(ref regex, id) = *self;
        let Grep(ref other_regex, other_id) = *other;

        regex.as_str() == other_regex.as_str() && id == other_id
    }
}

impl Eq for Grep {}

impl Hash for Grep {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Grep(ref regex, id) = *self;
        regex.as_str().hash(state);
        id.hash(state);
    }
}

impl Serialize for Grep {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let Grep(ref regex, UserId(id)) = *self;
        Serialize::serialize(&(regex.as_str(), id), serializer)
    }
}

impl<'de> Deserialize<'de> for Grep {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let (regex, id): (String, u64) = Deserialize::deserialize(deserializer)?;
        let regex = Regex::new(&regex)
            .map_err(|e| <D as Deserializer>::Error::custom(format!("{}", e)))?;
        let id = UserId(id);
        Ok(Grep(regex, id))
    }
}

