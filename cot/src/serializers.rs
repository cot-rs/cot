pub(crate) mod humantime {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    #[expect(clippy::ref_option, reason = "&Option<_> needed for serde")]
    pub(crate) fn serialize<S>(
        duration: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(duration) => {
                serializer.serialize_str(&humantime::format_duration(*duration).to_string())
            }
            None => serializer.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Option::<String>::deserialize(deserializer)?;
        match s {
            None => Ok(None),
            Some(value) => humantime::parse_duration(&value)
                .map(Some)
                .map_err(serde::de::Error::custom),
        }
    }
}

pub(crate) mod session_expiry_time {
    use chrono::DateTime;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::config::Expiry;

    pub(crate) fn serialize<S>(expiry: &Expiry, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match expiry {
            Expiry::OnSessionEnd => serializer.serialize_none(),
            Expiry::OnInactivity(time) => super::humantime::serialize(&Some(*time), serializer),
            Expiry::AtDateTime(time) => serializer.serialize_str(&time.to_string()),
        }
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Expiry, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Option::<String>::deserialize(deserializer)?;
        match s {
            None => Ok(Expiry::OnSessionEnd),
            Some(value) => {
                humantime::parse_duration(&value)
                    .map(Expiry::OnInactivity)
                    // On failure, fall back to RFC3339 format
                    .or_else(|_| {
                        DateTime::parse_from_rfc3339(&value)
                            .map(Expiry::AtDateTime)
                            .map_err(|e| {
                                serde::de::Error::custom(format!(
                                    "expiry must be a humantime duration or RFC3339 timestamp; got {value:?}: {e:?}"
                                ))
                            })
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::DateTime;
    use serde::Serialize;

    use crate::config::Expiry;

    #[derive(Serialize)]
    struct Wrapper {
        #[serde(with = "crate::serializers::session_expiry_time")]
        expiry: Expiry,
    }
    #[test]
    fn json_serialize_session_expiry_time() {
        let opts = [
            (
                Wrapper {
                    expiry: Expiry::OnSessionEnd,
                },
                r#"{"expiry":null}"#,
            ),
            (
                Wrapper {
                    expiry: Expiry::OnInactivity(Duration::from_secs(3600)),
                },
                r#"{"expiry":"1h"}"#,
            ),
            (
                Wrapper {
                    expiry: Expiry::AtDateTime(
                        DateTime::parse_from_rfc3339("2025-12-31T23:59:59+00:00").unwrap(),
                    ),
                },
                r#"{"expiry":"2025-12-31 23:59:59 +00:00"}"#,
            ),
        ];

        for (wrapper, expected) in opts {
            let json = serde_json::to_string(&wrapper).unwrap();
            assert_eq!(json, expected);
        }
    }
}
