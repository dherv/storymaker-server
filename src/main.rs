use actix_web::{web, App, HttpServer, Responder, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
struct Page {
    lines: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Novel {
    id: Uuid,
    title: String,
    pages: Vec<Vec<String>>,
}

async fn get_all() -> Result<impl Responder> {
    let paths = fs::read_dir("./novels/json").unwrap();
    let novels: Vec<Novel> = paths
        .map(|path| fs::read_to_string(path.as_ref().unwrap().path().display().to_string()))
        .map(|path| serde_json::from_str(&path.unwrap().to_string()).unwrap())
        .collect();
    Ok(web::Json(novels))
}

async fn get_one(id: web::Path<String>) -> Result<impl Responder> {
    let paths = fs::read_dir("./novels/json").unwrap();
    let result_path: Result<std::string::String, std::io::Error> = paths
        .map(|path| path.as_ref().unwrap().path().display().to_string())
        .filter(|path_string| path_string.contains(&id.to_string()))
        .map(|path| fs::read_to_string(path))
        .collect();
    let novel: Novel = serde_json::from_str(&result_path.unwrap().to_string())?;
    Ok(web::Json(novel))
}

#[derive(Deserialize)]
struct Info {
    id: Uuid,
    page: usize,
}

#[derive(Serialize)]
struct ErrMessage {
    message: String,
}

async fn get_one_page(info: web::Path<Info>) -> Result<impl Responder> {
    let paths = fs::read_dir("./novels/json").unwrap();
    let result_path: Result<std::string::String, std::io::Error> = paths
        .map(|path| path.as_ref().unwrap().path().display().to_string())
        .filter(|path_string| path_string.contains(&info.id.to_string()))
        .map(|path| fs::read_to_string(path))
        .collect();
    let novel: Novel = serde_json::from_str(&result_path.unwrap().to_string())?;

    let index = info.page - 1;
    let pages: Vec<Vec<String>> = novel.pages;
    // need to clone otherwise will create a reference in current scope and can not be passed to Ok(web::Json(page))
    let page = pages[index].clone();
    Ok(web::Json(page))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(
            web::scope("/api")
                .route("/", web::get().to(get_all))
                .route("/{id}", web::get().to(get_one))
                .route("/{id}/page/{page}", web::get().to(get_one_page)),
        )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
