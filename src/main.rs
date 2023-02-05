mod database;
use database::data;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{fs, time::Duration};

use actix_web_lab::web::spa;
use futures_util::StreamExt as _;
use std::future::Future;

use actix_web::{
    cookie::Cookie,
    get, post,
    web::{self, scope},
    App, FromRequest, HttpMessage, HttpResponse, HttpServer, Result,
};

#[post("/user")]
async fn get_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let user_id = {
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
    }
    .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?;

    println!("Fetching User:{}", user_id);

    std::thread::sleep_ms(1000);

    Ok(HttpResponse::Found().body(
        fs::read_to_string(format!("database/users/{}.json", user_id))
            .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?,
    ))
}

#[post("/newuser/{name}")]
async fn generate_user(path: web::Path<(String)>) -> Result<HttpResponse> {
    println!("{path}");
    let username = path.into_inner();
    let mut new_user = data::User::new();
    new_user.username = Some(username);
    new_user.push_to_data_base();

    let response = HttpResponse::Found().body(new_user.uuid);
    Ok(response)
}

#[derive(Serialize, Deserialize, Debug)]
struct InspectPost {
    user_uuid: String,
    inspection_to_post: data::Inspection,
}

#[post("/post-inspection")]
async fn add_inspection_to_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: InspectPost = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;

    // Load the user and append the inspection
    let mut inspectee = data::User::read_from_database(request.user_uuid)?;
    inspectee.push_inspection(request.inspection_to_post);
    inspectee.push_to_data_base();

    Ok(actix_web::HttpResponse::Ok().finish())
}

#[get("/inspections.json")]
async fn return_inspections() -> Result<HttpResponse> {
    let inspections = data::load_inspection_list()
        .map_err(|_| actix_web::error::ErrorInternalServerError("Internal Error Occured"))?;

    println!("Served inspection list");

    Ok(HttpResponse::Found()
        .body(serde_json::ser::to_string(&inspections).expect("This should always work")))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use actix_files as fs;
    HttpServer::new(|| {
        App::new()
            .service(
                scope("/api")
                    .service(get_user)
                    .service(generate_user)
                    .service(return_inspections)
                    .service(add_inspection_to_user)
                    .service(return_inspections),
            )
            .service(
                spa()
                    .index_file("../yew-front-end/dist/index.html")
                    .static_resources_mount("")
                    .static_resources_location("../yew-front-end/dist")
                    .finish(),
            )
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}
