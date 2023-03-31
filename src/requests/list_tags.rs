use std::{borrow::Cow, error::Error};

use askama::Template;

use crate::map::BidirectionalMap;

use super::{empty_as_none, parse, Parse, Request};

#[derive(Template)]
#[template(path = "load_tags.xml")]
pub struct ListTags;

impl Request for ListTags {
    fn method(&self) -> Cow<str> {
        "PROPFIND".into()
    }

    fn endpoint(&self) -> Cow<str> {
        "systemtags".into()
    }
}

impl Parse for ListTags {
    type Output = BidirectionalMap<u64, String>;

    fn parse(input: &str) -> Result<Self::Output, Box<dyn Error>> {
        let element: MultiStatus = parse(input)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_all_tags() {
        let input = include_str!("../../helper-scripts/all_tags.xml");
        let tags = ListTags::parse(input).unwrap();
        assert_eq!(tags.len(), 237);
        assert!(tags
            .iter()
            .any(|(&id, name)| id == 73 && name == "Architecture"))
    }
}