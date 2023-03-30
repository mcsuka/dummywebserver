#[macro_use]
extern crate rocket;

use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};

use dashmap::DashMap;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{ContentType, Status, Method, Header};
use rocket::request::{self, FromRequest, Request};
use rocket::response::status::NotFound;
use rocket::serde::{json::Json, Deserialize};
use rocket::tokio::time::{sleep, Duration};
use rocket::{State, Data, Response};

use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use rocket_okapi::request::{OpenApiFromRequest, RequestHeaderInput};
use rocket_okapi::settings::UrlObject;
use rocket_okapi::{openapi, openapi_get_routes, rapidoc::*, swagger_ui::*};
use serde::Serialize;

use rocket_db_pools::sqlx::{self, Row};
use rocket_db_pools::Database;

#[derive(Database)]
#[database("imdb_db")]
struct DbPool(sqlx::PgPool);

impl<'r> OpenApiFromRequest<'r> for &'r DbPool {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
struct TitleBasics {
    tconst: String,
    titletype: String,
    primarytitle: String,
    originaltitle: String,
    startyear: i32,
    runtimeminutes: i32,
    genres: String,
    isadult: bool,
    principals: Vec<TitlePrincipal>
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
struct TitlePrincipal {
    nconst: String,
    category: String,
    characters: String,
    primaryname: String,
    birthyear: i32,
    deathyear: i32
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
struct Greeting {
    text: String,
    id: u32,
}

struct AllGreetings {
    gr: DashMap<u32, String>,
}

struct AllGreetingsGuard<'r>(&'r DashMap<u32, String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AllGreetingsGuard<'r> {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, ()> {
        // Using `State` as a request guard. Use `inner()` to get an `'r`.
        request
            .guard::<&State<AllGreetings>>()
            .await
            .map(|allgreetings| AllGreetingsGuard(&allgreetings.gr))

        // Or alternatively, using `Rocket::state()`:
        // request.rocket().state::<AllGreetings>()
        //     .map(|allgreetings| AllGreetingsGuard(&allgreetings.gr))
        //     .or_forward(())
    }
}

impl<'r> OpenApiFromRequest<'r> for AllGreetingsGuard<'r> {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
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
fn greetings(all_greetings: AllGreetingsGuard) -> Json<Vec<Greeting>> {
    let greetings = all_greetings
        .0
        .iter()
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
        None => None,
    };
    format!("{:?}", x)
}

/**
Return *"Hello \<message\>!"*
*/
// #[openapi(tag = "Hello")]
// #[get("/<message>")]
// fn greet(message: &str) -> String {
//     format!("Hello, {}!", message)
// }

/**
Add a new greeting, identified by the `id`
*/
#[openapi(tag = "Hello")]
#[post("/", data = "<input>")]
fn new(all_greetings: &State<AllGreetings>, input: Json<Greeting>) -> (Status, String) {
    let x = all_greetings
        .gr
        .entry(input.id)
        .or_insert_with(|| input.text.to_owned());
    if x.value() == &input.text {
        (Status::Created, "".to_string())
    } else {
        (Status::Conflict, format!("Greeting with id {} already exists", input.id))
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

#[openapi(tag = "Async Process")]
#[get("/imdb/title/<id>")]
async fn imdb_title(db: &DbPool, id: &str) -> Result<Json<TitleBasics>, NotFound<String>> {
    println!("########### id = {id}");
    let result = sqlx::query("SELECT * FROM title_basics WHERE tconst = $1")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .and_then(|r| { 
            println!("########## row = {:?}", r.columns()); 
            Ok(TitleBasics {
                tconst: r.get::<String, &str>("tconst"),
                titletype: r.try_get::<String, &str>("titletype").unwrap_or("".to_string()),
                primarytitle: r.try_get::<String, &str>("primarytitle").unwrap_or("".to_string()),
                originaltitle: r.try_get::<String, &str>("originaltitle").unwrap_or("".to_string()),
                startyear: r.try_get::<i32, &str>("startyear").unwrap_or(0),
                runtimeminutes: r.try_get::<i32, &str>("runtimeminutes").unwrap_or(0),
                genres: r.try_get::<String, &str>("genres").unwrap_or("".to_string()),
                isadult: r.try_get::<bool, &str>("isadult").unwrap_or(false),
                principals: vec![]
            }) 
        })
        .ok();
    
    println!("########### result = {:?}", result);

    match result {
        Some(value) => {
            let mut new_value = value;
            let principals = 
            sqlx::query("SELECT tp.nconst, tp.category, tp.job, tp.characters , nb.primaryname, nb.birthyear, nb.deathyear
                FROM title_principals tp
                JOIN name_basics nb ON nb.nconst = tp.nconst
                WHERE tconst = $1")
                .bind(id)
                .fetch_all(&db.0)
                .await
                .and_then(|rows| { 
                    Ok(
                        rows.iter().map(|r| TitlePrincipal {
                        nconst: r.get::<String, &str>("nconst"),
                        category: r.try_get::<String, &str>("category").unwrap_or("".to_string()),
                        characters: r.try_get::<String, &str>("characters").unwrap_or("".to_string()),
                        primaryname: r.try_get::<String, &str>("primaryname").unwrap_or("".to_string()),
                        birthyear: r.try_get::<i32, &str>("birthyear").unwrap_or(0),
                        deathyear: r.try_get::<i32, &str>("deathyear").unwrap_or(0)
                    }).collect::<Vec<TitlePrincipal>>()
                )
                })
                .ok();
            
            if let Some(p) = principals {
                new_value.principals = p;
            }
            Ok(Json(new_value))
        }
        None => Err(NotFound(format!("Could not find title with id {id}"))),
    }
}


struct Counter {
    get: AtomicUsize,
    post: AtomicUsize,
}

impl Counter {
    fn new() -> Counter {
        Counter {
            get: AtomicUsize::new(0),
            post: AtomicUsize::new(0),
        }
    }
}

#[rocket::async_trait]
impl Fairing for Counter {
    // This is a request and response fairing named "GET/POST Counter".
    fn info(&self) -> Info {
        Info {
            name: "GET/POST Counter",
            kind: Kind::Request | Kind::Response
        }
    }

    // Increment the counter for `GET` and `POST` requests.
    async fn on_request(&self, request: &mut Request<'_>, _: &mut Data<'_>) {
        match request.method() {
            Method::Get => self.get.fetch_add(1, Ordering::Relaxed),
            Method::Post => self.post.fetch_add(1, Ordering::Relaxed),
            _ => return
        };
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        // Don't change a successful user's response body.
        if response.status() != Status::NotFound {
            response.set_header(Header::new("X-GET-Count", self.get.load(Ordering::Relaxed).to_string()));
            response.set_header(Header::new("X-POST-Count", self.post.load(Ordering::Relaxed).to_string()));
        } else 
        // Rewrite the response to return the current counts.
        if request.method() == Method::Get && request.uri().path() == "/counts" {
            let get_count = self.get.load(Ordering::Relaxed);
            let post_count = self.post.load(Ordering::Relaxed);
            let body = format!("Get: {}\nPost: {}\n", get_count, post_count);

            response.set_status(Status::Ok);
            response.set_header(ContentType::Plain);
            response.set_sized_body(body.len(), Cursor::new(body));
        }
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(AllGreetings { gr: DashMap::new() })
        .attach(DbPool::init())
        .attach(Counter::new())
        .mount(
            "/",
            openapi_get_routes![
                index, query, new, retrieve, teapot, greetings, delay, imdb_title
            ],
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
