use std::borrow::Cow;

use askama::Template;

use bimap::BiMap;
use reqwest::header::HeaderMap;

use crate::{Tag, TagId};

use super::{empty_as_none, parse, str_to_method, Body, DeserializeError, Parse, Request};

#[derive(Template)]
#[template(path = "load_tags.xml")]
pub struct ListTags;

impl Request for ListTags {
    fn method(&self) -> reqwest::Method {
        str_to_method("PROPFIND")
    }

    fn endpoint(&self) -> Cow<str> {
        "systemtags".into()
    }

    fn body(&self) -> Option<Body> {
        Some(self.into())
    }
}

impl Parse for ListTags {
    type Output = BiMap<TagId, Tag>;
    type Error = DeserializeError;

    fn parse(_: &HeaderMap, input: &str) -> Result<Self::Output, Self::Error> {
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

                let tag_name = prop.display_name.and_then(|n| Tag::new_or_log_error(&n));
                prop.id.zip(tag_name)
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
    id: Option<TagId>,
    display_name: Option<String>,
    #[serde(deserialize_with = "empty_as_none")]
    user_visible: Option<bool>,
    #[serde(deserialize_with = "empty_as_none")]
    user_assignable: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn deserialize_all_tags() {
        let input = include_str!("../../../test_data/all_tags.xml");
        let tags = ListTags::parse(&HeaderMap::new(), input).unwrap();
        let arch: Tag = "Architecture".parse().unwrap();
        assert_eq!(tags.len(), 237);
        assert!(tags
            .iter()
            .any(|(&id, name)| id == TagId::from(73) && name == &arch))
    }
}
