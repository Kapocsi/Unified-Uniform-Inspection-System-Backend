mod auth;
mod database;

use crate::auth::database::Token;
use actix_cors::Cors;
use actix_web::middleware::{Logger, NormalizePath};
use actix_web::{get, post, web, web::scope, App, HttpResponse, HttpServer, Result};
use actix_web_lab::web::spa;

use database::data::{self, index_users, read_user_index, Flight};

use futures_util::StreamExt as _;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use serde::{Deserialize, Serialize};
use std::{env, fs};

use dotenv::dotenv;
use env_logger::Env;

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

    let user = data::User::read_from_database(user_id)
        .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?;

    Ok(HttpResponse::Found().body(
        serde_json::to_string(&user)
            .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?,
    ))
}

#[get("/user/img/{user_id}.svg")]
async fn get_qrcode_for_user(path: web::Path<String>) -> Result<HttpResponse> {
    use qrcode::QrCode;

    let user_id = path.into_inner();
    match data::User::read_from_database(user_id.clone()).is_ok() {
        true => {
            let qr_code = QrCode::with_error_correction_level(
                format!("https://uniform.952aircadets.ca/u/{}", user_id).into_bytes(),
                qrcode::EcLevel::L,
            )
            .unwrap();
            // .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse uuid"))?;
            Ok(HttpResponse::Ok().content_type("image/svg+xml").body(
                qr_code
                    .render::<qrcode::render::svg::Color>()
                    .min_dimensions(300, 300)
                    .build(),
            ))
        }
        false => Err(actix_web::error::ErrorBadRequest("User not found")),
    }
}

#[post("validate_uuid/{uuid}")]
async fn validate_uuid(path: web::Path<String>) -> Result<HttpResponse> {
    use std::path::Path;

    let user_id = path.into_inner();

    let exitst = Path::new(format!("database/users/{user_id}.json").as_str()).exists();
    let response = match exitst {
        true => "true",
        false => "false",
    };

    Ok(HttpResponse::Ok().body(response))
}

#[get("/newuser/")]
async fn generate_user() -> Result<HttpResponse> {
    let new_user = data::User::new();
    new_user.push_to_data_base();

    let response = HttpResponse::Found()
        .append_header(("location", format!("/u/{}", new_user.uuid)))
        .body(new_user.uuid);
    Ok(response)
}

#[post("/newuser/")]
async fn post_generate_user() -> Result<HttpResponse> {
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

            Ok(actix_web::HttpResponse::Ok().body(serde_json::to_string(&read_user_index()?)?))
        }
        auth_database::TokenResponse::Invalid => {
            Err(actix_web::error::ErrorForbidden("Invalid Token"))
        }
        auth_database::TokenResponse::Expired => {
            Err(actix_web::error::ErrorForbidden("Token Expired"))
        }
    }

    // Load the user and append the inspection
}

#[get("/inspections.json")]
async fn return_inspections() -> Result<HttpResponse> {
    let mut inspections = data::load_inspection_list()
        .map_err(|_| actix_web::error::ErrorInternalServerError("Internal Error Occured"))?;

    inspections.iter_mut().for_each(|f| f.compute_score());

    Ok(HttpResponse::Found()
        .body(serde_json::ser::to_string(&inspections).expect("This should always work")))
}

use auth::database::{self as auth_database, TokenResponse};

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

#[actix_web::get("/{url:.*}")]
async fn http_upgrade() -> Result<HttpResponse> {
    let file_string = fs::read_to_string("../UUIS-backend/https_upgrade.html")
        .expect("https upgrade screen missing")
        .replace("\n", "");

    Ok(HttpResponse::Found()
        .content_type("text/html")
        .insert_header(("redirect", "https://uniform.952aircadets.ca"))
        .body(file_string))
}

#[derive(Deserialize)]
struct SetFlight {
    uuid: String,
    flight: data::Flight,
}

#[post("/set_flight")]
async fn set_flight(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: SetFlight = serde_json::de::from_str({
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

    user.flight = Some(request.flight);
    user.push_to_data_base();

    Ok(HttpResponse::Ok().finish())
}

#[post("/user_index")]
async fn user_index(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: Token = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;

    match request.check_token_validy() {
        TokenResponse::Valid => {
            Ok(HttpResponse::Ok().body(serde_json::ser::to_string(&read_user_index()?)?))
        }
        TokenResponse::Invalid => Err(actix_web::error::ErrorForbidden("Invalid Token")),
        TokenResponse::Expired => Err(actix_web::error::ErrorForbidden("Expired Token")),
    }
}

#[derive(Deserialize, Clone)]
struct BulkUserRequest {
    token: Token,
    flight: Option<Flight>,
    names: Vec<Option<String>>,
}

#[post("/bulk-new-user")]
async fn bulk_new_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let request: BulkUserRequest = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;

    // Check if the token is valid, early return if it is not
    let _ = match request.token.check_token_validy() {
        TokenResponse::Expired | TokenResponse::Invalid => {
            Err(actix_web::error::ErrorForbidden("Token Invalid or Expired"))
        }
        TokenResponse::Valid => Ok(()),
    }?;

    // Generate new users
    let new_users: Vec<data::FlightIndexItem> = request
        .names
        .iter()
        .map(|n| {
            let mut new_user = data::User::new();
            new_user.username = n.clone();
            new_user.flight = (&request.flight).clone();
            new_user.push_to_data_base();
            new_user.dev_user = true;
            new_user.into()
        })
        .collect();

    Ok(HttpResponse::Ok().body(serde_json::ser::to_string(&new_users)?))
}

#[get("/flight_list")]
async fn serve_flight_list() -> HttpResponse {
    let flights: [String; 5] = [
        "Beddoe".into(),
        "Morgan".into(),
        "Spear".into(),
        "Bell".into(),
        "Hill".into(),
    ];
    HttpResponse::Found().body(serde_json::to_string(&flights).unwrap())
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    index_users()?;
    // initlize the .env file
    dotenv().ok();

    let private_key_path = env::var("PRIVKEY").unwrap();
    let cert_path = env::var("CERT").unwrap();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let mut ssl_builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    ssl_builder
        .set_private_key_file(private_key_path, SslFiletype::PEM)
        .unwrap();
    ssl_builder.set_certificate_chain_file(cert_path).unwrap();

    let secure_server = HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(
                scope("/api")
                    .service(get_user)
                    .service(generate_user)
                    .service(return_inspections)
                    .service(add_inspection_to_user)
                    .service(return_inspections)
                    .service(signup)
                    .service(login)
                    .service(claim_user)
                    .service(validate_uuid)
                    .service(post_generate_user)
                    .service(get_qrcode_for_user)
                    .service(serve_flight_list)
                    .service(set_flight)
                    .service(bulk_new_user)
                    .service(user_index),
            )
            .service(
                spa()
                    .index_file("../front-end/build/index.html")
                    .static_resources_mount("/")
                    .static_resources_location("../front-end/../front-end/build")
                    .finish(),
            )
            .wrap(
                Cors::default()
                    .allowed_origin("https://uniform.952aircadets.ca")
                    .allowed_origin("https://beta.uniform.kapocsi.ca")
                    .allowed_origin("http://127.0.0.1:5173"),
            )
            .wrap(NormalizePath::trim())
    });

    let server = HttpServer::new(|| App::new().service(http_upgrade));

    // For testing on a local machine, if compliled in debug the server will run on http for the
    // "secure" side
    #[cfg(debug_assertions)]
    {
        let secure_server = secure_server.bind("0.0.0.0:8080")?.run();
        let server = server.bind("0.0.0.0:8090")?.run();

        let (secure_server_result, server_result) = futures::join!(server, secure_server);
        secure_server_result?;
        server_result?;
    }

    #[cfg(not(debug_assertions))]
    {
        let secure_server = secure_server
            .bind_openssl("0.0.0.0:443", ssl_builder)?
            .run();
        let server = server.bind("0.0.0.0:80")?.run();

        let (secure_server_result, server_result) = futures::join!(server, secure_server);
        secure_server_result?;
        server_result?;
    }

    Ok(())
}
