#[macro_use]
extern crate rocket;

use std::collections::HashMap;

use rocket::serde::{json::Json, Deserialize};
use rocket::tokio::time::{sleep, Duration};

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use rocket_okapi::settings::UrlObject;
use rocket_okapi::{openapi, openapi_get_routes, rapidoc::*, swagger_ui::*};

#[derive(Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
struct Greeting<'r> {
    text: &'r str,
    id: u32,
}

static mut GREETINGS: Option<HashMap<u32, String>> = None;

/// Returns `"Hello World!"`
#[openapi(tag = "Hello")]
#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

/**
 Return the message provided in the query string
 */
#[openapi(tag = "Hello")]
#[get("/query?<text>")]
fn query(text: &str) -> String {
    format!("{}", text)
}

/**
 ***Retrieve*** a **message** *inserted* by `POST`
 */
#[openapi(tag = "Hello")]
#[get("/retrieve/<id>")]
fn retrieve(id: u32) -> String {
    unsafe {
        let x = GREETINGS.as_ref().unwrap().get(&id);
        format!("{:?}", x)
    }
}

/**
 Return *"Hello \<message\>!"*
 */
#[openapi(tag = "Hello")]
#[get("/<message>")]
fn greet(message: &str) -> String {
    format!("Hello, {}!", message)
}

/**
 Add a new message, identified by the id
 */
#[openapi(tag = "Hello")]
#[post("/", data = "<input>")]
fn new(input: Json<Greeting<'_>>) {
    unsafe {
        match &mut GREETINGS {
            Some(gr) => {
                gr.insert(input.id, input.text.to_string());
            }
            None => panic!("greetings must be set up!"),
        }
    }
}

/**
 Asynchronous process waiting n seconds before returning
 */
#[openapi(tag = "Async Process")]
#[get("/delay/<seconds>")]
 async fn delay(seconds: u64) -> String {
     sleep(Duration::from_secs(seconds)).await;
     format!("Waited for {} seconds", seconds)
 }

#[launch]
fn rocket() -> _ {
    unsafe {
        GREETINGS = Some(HashMap::new());
    }
    rocket::build()
        .mount("/", openapi_get_routes![index, delay, greet, query, new, retrieve])
        .mount(
            "/swagger-ui/",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../openapi.json".to_owned(),
                ..Default::default()
            }),
        )
        .mount(
            "/rapidoc/",
            make_rapidoc(&RapiDocConfig {
                title: Some("Sandbox Webserver with Rust/Rocket".to_owned()),
                general: GeneralConfig {
                    spec_urls: vec![UrlObject::new("General", "../openapi.json")],
                    ..Default::default()
                },
                hide_show: HideShowConfig {
                    allow_spec_url_load: false,
                    allow_spec_file_load: false,
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
}
