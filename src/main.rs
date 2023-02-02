mod database;
use database::data::{self, AuthenticatedUser};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{fs, time::Duration};

use futures_util::StreamExt as _;
use std::future::Future;

use actix_web::{
    cookie::Cookie, get, post, web, App, FromRequest, HttpResponse, HttpServer, Result,
};

#[post("/api/user")]
async fn get_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let user_id = {
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
    }
    .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?;

    Ok(HttpResponse::Found().body(
        fs::read_to_string(format!("database/users/{}.json", user_id))
            .map_err(|_| actix_web::error::ErrorNotFound("User not found"))?,
    ))
}

#[post("/api/newuser/{name}")]
async fn generate_user(path: web::Path<(String)>) -> Result<HttpResponse> {
    println!("{path}");
    let username = path.into_inner();
    let mut new_user = data::User::new();
    new_user.username = Some(username);
    new_user.push_to_data_base();

    let response = HttpResponse::Found().body(new_user.uuid);
    Ok(response)
}

#[derive(Serialize, Deserialize)]
struct UserCredentials {
    username: String,
    password: String,
}

#[post("/api/auth/newuser")]
async fn new_auth_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let new_user: UserCredentials = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;
    data::AuthenticatedUser::new(new_user.username, new_user.password).push_to_data_base();

    Ok(HttpResponse::Ok().finish())
}

:#[post("/api/auth/login")]
async fn authenticate_user(mut payload: web::Payload) -> Result<HttpResponse> {
    let new_user: UserCredentials = serde_json::de::from_str({
        let mut bytes = web::BytesMut::new();
        while let Some(item) = payload.next().await {
            bytes.extend_from_slice(&item?);
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| actix_web::error::ErrorBadRequest("Could not parse request"))?
            .as_str()
    })?;

    let mut user: data::AuthenticatedUser =
        data::AuthenticatedUser::read_from_data_base(new_user.username, new_user.password)
            .map_err(|_| actix_web::error::ErrorNotFound("Username or password incorrect"))?;

    user.attach_token();


    println!("{:?}", user.auth_token);
    let cookieString = serde_json::ser::to_string(&user.auth_token).unwrap();
    user.push_to_data_base();

    Ok(HttpResponse::Found()
        .body(cookieString)
        .add_cookie(Cookie::new("token", cookieStr)
}

#[get("/printer")]
async fn printer(mut payload: web::Payload) -> Result<HttpResponse> {
    let mut bytes = web::BytesMut::new();
    while let Some(item) = payload.next().await {
        bytes.extend_from_slice(&item?);
    }
    println!("{:?}", bytes);

    Ok(HttpResponse::Ok().body(bytes))
}

#[derive(Serialize, Deserialize, Debug)]
struct InspectPost {
    user_uuid: String,
    inspection_to_post: data::Inspection,
    auth_token: data::AuthToken,
}

#[post("/api/post-inspection")]
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

    // Check if user is authorized, we early return if not we continue
    match request.auth_token.is_valid() {
        Ok(true) => Ok(true),
        Err(_) | Ok(false) => Err(actix_web::error::ErrorUnauthorized("Not Authorized")),
    }?;

    // Load the user and append the inspection
    let mut inspectee = data::User::read_from_database(request.user_uuid)?;
    inspectee.push_inspection(request.inspection_to_post);
    inspectee.push_to_data_base();

    Ok(actix_web::HttpResponse::Ok().finish())
}

#[get("/api/inspections.json")]
async fn return_inspections() -> Result<HttpResponse> {
    let inspections = data::load_inspection_list()
        .map_err(|_| actix_web::error::ErrorInternalServerError("Internal Error Occured"))?;
    Ok(HttpResponse::Found()
        .body(serde_json::ser::to_string(&inspections).expect("This should always work")))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let test = InspectPost {
        user_uuid: "UUID".to_owned(),
        inspection_to_post: data::Inspection {
            name: "Blues".to_owned(),
            criteria: vec![
                data::Criteria::Graded(data::CriteriaGraded {
                    category_name: "Category Name".to_string(),
                    descriptions: vec!["Description 1".to_string()],
                    state: Some(1),
                }),
                data::Criteria::PassFail(data::CriteriaPassFail {
                    description: "Pass Fail".to_string(),
                    state: Some(false),
                    category_name: "Category Name".to_string(),
                }),
                data::Criteria::Comment(Some("A comment".to_string())),
            ],
            date: Some(0),
        },
        auth_token: {
            let mut user = AuthenticatedUser::new("Thomas".to_string(), "1234".to_string());
            user.attach_token();
            user.push_to_data_base();
            let user =
                AuthenticatedUser::read_from_data_base("Thomas".to_string(), "1234".to_string())
                    .unwrap();
            user.auth_token.unwrap()
        },
    };

    println!("{}", serde_json::to_string(&test).unwrap());

    HttpServer::new(|| {
        App::new()
            .service(get_user)
            .service(generate_user)
            .service(new_auth_user)
            .service(authenticate_user)
            .service(add_inspection_to_user)
            .service(return_inspections)
            .service(printer)
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}
