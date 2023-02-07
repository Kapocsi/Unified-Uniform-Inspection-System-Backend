mod auth;
mod database;

use actix_web::web::service;
use database::data;
use serde::{Deserialize, Serialize};

use actix_web_lab::web::spa;
use futures_util::StreamExt as _;

use actix_web::{get, post, web, web::scope, App, HttpResponse, HttpServer, Result};
use std::fs;

use std::string;

#[post("/user")]
async fn get_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let user_id = {
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
    }
    .map_err(|_| actix_web::error::ErrorBadRequest("could not parse request"))?;

    println!("Fetching User:{}", user_id);

    let user = data::User::read_from_database(user_id)
        .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?;

    Ok(HttpResponse::Found().body(
        serde_json::to_string(&user)
            .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?,
    ))
}

#[post("/newuser/")]
async fn generate_user() -> Result<HttpResponse> {
    let new_user = data::User::new();
    new_user.push_to_data_base();

    let response = HttpResponse::Found().body(new_user.uuid);
    Ok(response)
}

#[derive(Serialize, Deserialize, Debug)]
struct InspectPost {
    user_uuid: String,
    inspection_to_post: data::Inspection,
    token: auth_database::Token,
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

    match request.token.check_token_validy() {
        auth_database::TokenResponse::Valid => {
            let mut inspectee = data::User::read_from_database(request.user_uuid)
                .map_err(|_| actix_web::error::ErrorNotFound("Requested Auth User Not Found"))?;

            inspectee.push_inspection(request.inspection_to_post);
            inspectee.push_to_data_base();

            Ok(actix_web::HttpResponse::Ok().finish())
        }
        auth_database::TokenResponse::Invalid => Ok(actix_web::HttpResponse::Forbidden().finish()),
        auth_database::TokenResponse::Expired => {
            Ok(actix_web::HttpResponse::Unauthorized().finish())
        }
    }

    // Load the user and append the inspection
}

#[get("/inspections.json")]
async fn return_inspections() -> Result<HttpResponse> {
    let inspections = data::load_inspection_list()
        .map_err(|_| actix_web::error::ErrorInternalServerError("Internal Error Occured"))?;

    println!("Served inspection list");

    Ok(HttpResponse::Found()
        .body(serde_json::ser::to_string(&inspections).expect("This should always work")))
}

use auth::database as auth_database;

#[derive(Serialize, Deserialize, Debug)]
struct UserLogin {
    username: String,
    password: String,
}

#[post("/auth/login")]
async fn login(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: UserLogin = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }

        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;

    println!("{:?}", request);

    Ok(
        match auth_database::User::get_user(request.username, request.password) {
            None => HttpResponse::NotFound().finish(),
            Some(mut t) => {
                t.accosiate_token();
                t.clone().push_to_disk();
                HttpResponse::Found().body(
                    serde_json::to_string(
                        t.tokens
                            .last()
                            .expect("We just added a token so we should be good here"),
                    )
                    .unwrap(),
                )
            }
        },
    )
}

#[post("/auth/signup")]
async fn signup(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: UserLogin = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;

    let user = auth_database::User::new(request.username, request.password)
        .map_err(|_| actix_web::error::ErrorLocked("Username taken"))?;

    user.push_to_disk();

    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct UserClaim {
    uuid: String,
    username: String,
}

#[post("/claim-user")]
async fn claim_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: UserClaim = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;
    let mut user = data::User::read_from_database(request.uuid)
        .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?;

    println!(
        "setting {:?} as username for {:?}",
        user.username, user.uuid
    );

    match user.username {
        None => {
            user.username = Some(request.username);
            user.push_to_data_base();
            Ok(HttpResponse::Ok().finish())
        }
        _ => Err(actix_web::error::ErrorForbidden(
            "Forbiden username already set",
        )),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(
                scope("/api")
                    .service(get_user)
                    .service(generate_user)
                    .service(return_inspections)
                    .service(add_inspection_to_user)
                    .service(return_inspections)
                    .service(signup)
                    .service(login)
                    .service(claim_user),
            )
            .service(
                spa()
                    .index_file("../yew-front-end/dist/index.html")
                    .static_resources_mount("")
                    .static_resources_location("../yew-front-end/dist")
                    .finish(),
            )
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await
}
