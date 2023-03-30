use std::error::Error;

use crate::{map::BidirectionalMap, requests::ListTags};

pub trait Parse {
    type Output;
    fn parse(input: &str) -> Result<Self::Output, Box<dyn Error>>;
}

impl Parse for ListTags {
    type Output = BidirectionalMap<u64, String>;

    fn parse(input: &str) -> Result<Self::Output, Box<dyn Error>> {
        let deserializer = &mut quick_xml::de::Deserializer::from_str(input);
        let element: MultiStatus = serde_path_to_error::deserialize(deserializer)?;
        Ok(element
            .props
            .into_iter()
            .filter_map(|prop| {
                let visible = prop.user_visible.unwrap_or_default();
                let assignable = prop.user_assignable.unwrap_or_default();
                if !visible || !assignable {
                    return None;
                }

                prop.id.zip(prop.display_name)
            })
            .collect())
    }
}

#[derive(Debug, serde_query::Deserialize)]
struct MultiStatus {
    #[query(".response.[].propstat.prop")]
    props: Vec<Prop>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
struct Prop {
    #[serde(deserialize_with = "empty_as_none")]
    id: Option<u64>,
    display_name: Option<String>,
    #[serde(deserialize_with = "empty_as_none")]
    user_visible: Option<bool>,
    #[serde(deserialize_with = "empty_as_none")]
    user_assignable: Option<bool>,
}

fn empty_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match serde::Deserialize::deserialize(de)? {
        None | Some("") => Ok(None),
        Some(s) => s.parse().map(Some).map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_all_tags() {
        let input = include_str!("../helper-scripts/all_tags.xml");
        let tags = ListTags::parse(input).unwrap();
        assert_eq!(tags.len(), 237);
        assert!(tags
            .iter()
            .any(|(&id, name)| id == 73 && name == "Architecture"))
    }
}
