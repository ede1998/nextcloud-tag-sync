use std::error::Error;

mod deserializers;
mod map;

#[tokio::main]
async fn main() {
    async fn m() -> Result<(), Box<dyn Error>> {
        let tags = load_tags_from_network().await;
        let tags = deserializers::deserialize_tags(&tags)?;

        println!("{tags}");
        Ok(())
    }
    match m().await {
        Ok(()) => {}
        Err(e) => println!("{e}"),
    }
}

async fn load_tags_from_network() -> String {
    let client = reqwest::Client::new();
    client
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
            "https://cloud.erik-hennig.me/remote.php/dav/systemtags",
        )
        .basic_auth(USER, Some(TOKEN))
        .body(
            "<?xml version=\"1.0\" encoding=\"utf-8\" ?>
                <a:propfind xmlns:a=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\">
	            <a:prop>
	                <oc:display-name/>
	                <oc:user-visible/>
	                <oc:user-assignable/>
	                <oc:id/>
        	    </a:prop>
        	</a:propfind>",
        )
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

const USER: &str = "erik";
const TOKEN: &str = include_str!("../helper-scripts/nextcloud-token.txt");
