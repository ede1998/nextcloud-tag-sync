use core::str;
use std::borrow::Cow;

use nextcloud_tag_sync::{
    parse, Body, DeserializeError, FileId, ListTagsMultiStatus, Parse, Request, Tags,
};
use reqwest::header::HeaderMap;

pub struct GetFileTags(pub FileId);

impl Request for GetFileTags {
    fn method(&self) -> reqwest::Method {
        reqwest::Method::from_bytes(b"PROPFIND").expect("valid HTTP method")
    }

    fn endpoint(&self) -> Cow<str> {
        format!("systemtags-relations/files/{}", self.0).into()
    }

    fn body(&self) -> Body {
        let content = r#"<?xml version="1.0" encoding="utf-8" ?>
          <a:propfind xmlns:a="DAV:" xmlns:oc="http://owncloud.org/ns">
	        <a:prop>
	          <oc:display-name/>
	          <oc:user-visible/>
	          <oc:user-assignable/>
	          <oc:id/>
	        </a:prop>
	      </a:propfind>"#;
        Body::Askama {
            content: Ok(content.to_owned()),
            mime_type: "application/xml",
        }
    }
}

impl Parse for GetFileTags {
    type Output = Tags;
    type Error = DeserializeError;

    fn parse(_: &HeaderMap, input: &str) -> Result<Self::Output, Self::Error> {
        let element: ListTagsMultiStatus = parse(input)?;

        Ok(element
            .props
            .into_iter()
            .filter_map(|prop| {
                let visible = prop.user_visible.unwrap_or_default();
                let assignable = prop.user_assignable.unwrap_or_default();
                if !visible || !assignable {
                    return None;
                }

                prop.display_name
            })
            .collect())
    }
}
