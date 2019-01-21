extern crate fxa_client;
#[macro_use] extern crate text_io;
extern crate url;

use fxa_client::{Config, FirefoxAccount};
use std::collections::HashMap;
use url::Url;

static CONTENT_SERVER: &'static str = "http://127.0.0.1:3030";
//static CONTENT_SERVER: &'static str = "https://pairsona2.dev.lcip.org";
//static CLIENT_ID: &'static str = "7f368c6886429f19";
//static REDIRECT_URI: &'static str = "https://mozilla.github.io/notes/fxa/android-redirect.html";
static CLIENT_ID: &'static str = "3c49430b43dfba77";
static REDIRECT_URI: &'static str = "https://accounts.firefox.com/oauth/success/3c49430b43dfba77";
static SCOPES: &'static [&'static str] = &["https://identity.mozilla.com/apps/oldsync"];

fn main() {
    let config = Config::import_from(CONTENT_SERVER).unwrap();
    let mut fxa = FirefoxAccount::new(config, CLIENT_ID, REDIRECT_URI);
    println!("Give me the pairing URL:");
    let pairing_url: String = read!("{}\n");
    let url = fxa.begin_pairing_flow(&pairing_url, &SCOPES).unwrap();
    println!("Open this URL in a browser:");
    println!("{}", url);
    println!("Give me the code:");
    let code: String = read!("{}\n");
    println!("Give me the state:");
    let state: String = read!("{}\n");
    let oauth_info = fxa.complete_oauth_flow(&code, &state).unwrap();
    println!("token keys: {}", oauth_info.keys.unwrap());
}