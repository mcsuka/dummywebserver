#[macro_use]
extern crate rocket;

use dashmap::DashMap;

use rocket::State;
use rocket::http::{ContentType, Status};
use rocket::serde::{json::Json, Deserialize};
use rocket::tokio::time::{sleep, Duration};

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use rocket_okapi::settings::UrlObject;
use rocket_okapi::{openapi, openapi_get_routes, rapidoc::*, swagger_ui::*};
use serde::Serialize;

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
struct Greeting {
    text: String,
    id: u32,
}

struct AllGreetings {
    gr: DashMap<u32, String>,
}

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

/// I'm a teapot
#[openapi(tag = "Hello")]
#[get("/teapot")]
fn teapot() -> (Status, (ContentType, &'static str)) {
    (Status::ImATeapot, (ContentType::JSON, "{ \"status\": 418,  \"description\": \"the server refuses to brew coffee because it is, permanently, a teapot\"}"))
}

/// List all greetings
#[openapi(tag = "Hello")]
#[get("/greetings")]
fn greetings(all_greetings: &State<AllGreetings>) -> Json<Vec<Greeting>> {
    let greetings = all_greetings.gr.iter()
        .map(|entry| Greeting {
            id: *entry.key(),
            text: entry.value().to_owned(),
        })
        .collect();

    Json(greetings)
}

/**
 ***Retrieve*** a **greeting** *with* by `id`
 */
#[openapi(tag = "Hello")]
#[get("/greetings/<id>")]
fn retrieve(all_greetings: &State<AllGreetings>, id: u32) -> String {
    let x = match all_greetings.gr.get(&id) {
        Some(rref) => Some(rref.value().to_owned()),
        None => None
    };
    format!("{:?}", x)
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
Add a new greeting, identified by the `id`
*/
#[openapi(tag = "Hello")]
#[post("/", data = "<input>")]
fn new(all_greetings: &State<AllGreetings>, input: Json<Greeting>) -> (Status, &'static str) {
    let x = all_greetings.gr.entry(input.id).or_insert_with(|| input.text.to_owned());
    if x.value() == &input.text {
        (Status::Created, "")
    } else {
        (Status::Conflict, "Greeting with id already exists")
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
    rocket::build()
        .manage(AllGreetings { gr: DashMap::new() })
        .mount(
            "/",
            openapi_get_routes![index, delay, greet, query, new, retrieve, teapot, greetings],
        )
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
