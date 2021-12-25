extern crate dotenv;
extern crate r2d2;
extern crate r2d2_mysql;
use actix_multipart::Multipart;
use actix_web::{
    middleware,
    web::{self},
    App, Error, HttpResponse, HttpServer, Responder, Result,
};
use futures_util::{StreamExt, TryStreamExt};
use mysql::{params, prelude::Queryable, Row};
use mysql::{Opts, OptsBuilder};
use r2d2_mysql::MysqlConnectionManager;
use serde::{Deserialize, Serialize};
use std::{env, fs, io::Write};
use uuid::Uuid;

use dotenv::dotenv;
type DbPool = r2d2::Pool<MysqlConnectionManager>;

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
#[derive(Deserialize, Serialize, Debug)]

struct NovelRow {
    id: usize,
    uuid: String,
    title: String,
    category: String,
    filename: String,
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
#[derive(Clone)]
struct FormData {
    title: Option<String>,
    category: Option<String>,
    filename: Option<String>,
}

impl FormData {
    fn new() -> FormData {
        FormData {
            title: None,
            category: None,
            filename: None,
        }
    }
}

async fn save_file(pool: web::Data<DbPool>, mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    let mut form_data = FormData::new();
    while let Some(mut field) = payload.try_next().await? {
        // A multipart/form-data stream has to contain `content_disposition`
        let content_disposition = field
            .content_disposition()
            .ok_or_else(|| HttpResponse::BadRequest().finish())?;

        let filename = content_disposition.get_filename();

        match filename {
            Some(name) => {
                form_data.filename = Some(name.to_string());
                let filepath = format!("./novels/{}", name);

                // File::create is blocking operation, use threadpool
                let mut f = web::block(|| std::fs::File::create(filepath)).await?;

                // Field in turn is stream of *Bytes* object
                while let Some(chunk) = field.try_next().await? {
                    // filesystem operations are blocking, we have to use threadpool
                    f = web::block(move || f.write_all(&chunk).map(|_| f)).await?;
                }
            }
            None => {
                let name = content_disposition.get_name().unwrap();
                while let Some(chunk) = field.next().await {
                    // TODO: check if it is okay to just read chunk here, since we await for it.
                    // it populate each field at a time
                    // println!("-- CHUNK: \n{:?}", std::str::from_utf8(&chunk?));
                    match std::str::from_utf8(&chunk?) {
                        Ok(c) => {
                            println!("result, {:?}, {:?}", name, c);
                            if name == "title" {
                                form_data.title = Some(c.to_string());
                            }
                            if name == "category" {
                                form_data.category = Some(c.to_string());
                            }
                        }
                        Err(err) => {
                            println!("err: {:?}", err)
                        }
                    }
                    // name - chunk in mysql
                }
            }
        }
    }

    let res = web::block(move || {
        let mut conn = pool.get().expect("could not connect to the db pool");
        let uuid = format!("{}", uuid::Uuid::new_v4());
        conn.exec_drop(
            r"INSERT INTO novel (uuid, title, category, filename) VALUES (:uuid, :title, :category, :filename)",
            params! {
                "uuid" => &uuid,
                "title" => form_data.title.unwrap(),
                "category" => form_data.category.unwrap(),
                "filename" => form_data.filename.unwrap(),
            },
        )
        .unwrap();

        conn.exec_map(r"SELECT * FROM novel WHERE uuid=:uuid", params! { "uuid" => &uuid }, |r: Row| {
            let mut row = r.clone();
            NovelRow { id: row.take("id").unwrap(),  uuid: row.take("uuid").unwrap(), category: row.take("category").unwrap(), title: row.take("title").unwrap(), filename: row.take("filename").unwrap()}
        })
    })
    .await
    .map(|user| HttpResponse::Ok().json(user))
    .map_err(|_| HttpResponse::InternalServerError())?;
    Ok(res)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let url = env::var("DATABASE_URL").unwrap();
    let opts = Opts::from_url(&url).unwrap();
    let builder = OptsBuilder::from_opts(opts);
    let manager = MysqlConnectionManager::new(builder);
    let pool = r2d2::Pool::builder().max_size(4).build(manager).unwrap();

    let mut conn = pool.get().expect("could not connect to db pool");
    // TODO: remove default from word and read-time and add not null to synopsis. replace filename by s3 url
    conn.query_drop(
        r"CREATE TABLE IF NOT EXISTS novel (
            id INT AUTO_INCREMENT PRIMARY KEY,
            uuid VARCHAR(255) NOT NULL UNIQUE,
            title VARCHAR(255) NOT NULL,
            category VARCHAR(255) NOT NULL,
            filename VARCHAR(255) NOT NULL,
            page_count TINYINT NOT NULL DEFAULT 0,
            word_count TINYINT NOT NULL DEFAULT 0,
            view_count TINYINT NOT NULL DEFAULT 0,
            like_count TINYINT NOT NULL DEFAULT 0,
            read_time TINYINT NOT NULL DEFAULT 0,
            synopsis TEXT,
            published_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        )",
    )
    .expect("could not create the table");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .data(pool.clone())
            .service(
                web::scope("/api")
                    .route("/", web::get().to(get_all))
                    .route("/{id}", web::get().to(get_one))
                    .route("/{id}/page/{page}", web::get().to(get_one_page))
                    .route("/admin/upload", web::post().to(save_file)),
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
