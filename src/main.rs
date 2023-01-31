extern crate dotenv;
extern crate r2d2;
extern crate r2d2_mysql;
extern crate regex;
use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::{
    middleware,
    web::{self},
    App, Error, HttpResponse, HttpServer, Responder, Result,
};
use dotenv::dotenv;
use futures_util::{StreamExt, TryStreamExt};
use mysql::{params, prelude::Queryable, Row};
use mysql::{Opts, OptsBuilder};
use r2d2_mysql::MysqlConnectionManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{env, fs, io::Write};
use uuid::Uuid;
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
    synopsis: String,
    page_count: usize,
    word_count: usize
}

async fn get_all() -> Result<impl Responder> {
    // read metadata
    // get file by name in DB
    // build the json with DB + text + pages
    let paths = fs::read_dir("./novels/json").unwrap();
    let novels: Vec<Novel> = paths
        .map(|path| fs::read_to_string(path.as_ref().unwrap().path().display().to_string()))
        .map(|path| serde_json::from_str(&path.unwrap().to_string()).unwrap())
        .collect();
    Ok(web::Json(novels))
}

async fn get_all_meta(pool: web::Data<DbPool>) -> Result<impl Responder> {
    // read metadata
    // get file by name in DB
    // build the json with DB + text + pages
    let res = web::block(move || {
        let mut conn = pool.get().expect("could not connect to the db pool");

        conn.query_map(r"SELECT * FROM novel", |mut row: Row| {
            // TODO: use ::new to create the NovelRow
            NovelRow { 
                id: row.take("id").unwrap(),  
                uuid: row.take("uuid").unwrap(), 
                category: row.take("category").unwrap(), 
                title: row.take("title").unwrap(),
                filename: row.take("filename").unwrap(), 
                synopsis: row.take("synopsis").unwrap(),
                page_count: row.take("page_count").unwrap(),
                word_count: row.take("word_count").unwrap(),
            }
        })
    })
    .await
    .map(|novel| HttpResponse::Ok().json(novel))
    .map_err(|_| HttpResponse::InternalServerError())?;
    Ok(res)

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
    id: String,
    page: usize,
}

#[derive(Serialize)]
struct ErrMessage {
    message: String,
}

async fn get_one_page(pool: web::Data<DbPool>, info: web::Path<Info>) -> Result<impl Responder> {
    // let paths = fs::read_dir("./novels/json").unwrap();
    // let result_path: Result<std::string::String, std::io::Error> = paths
    //     .map(|path| path.as_ref().unwrap().path().display().to_string())
    //     .filter(|path_string| path_string.contains(&info.id.to_string()))
    //     .map(|path| fs::read_to_string(path))
    //     .collect();
    // let novel: Novel = serde_json::from_str(&result_path.unwrap().to_string())?;

    // let index = info.page - 1;
    // let pages: Vec<Vec<String>> = novel.pages;
    // // need to clone otherwise will create a reference in current scope and can not be passed to Ok(web::Json(page))
    // let page = pages[index].clone();
    // Ok(web::Json(page))
    let id = info.id.to_string();

    println!("{}", id);
    let res = web::block(move || {
       
        let mut conn = pool.get().expect("could not connect to the db pool");

        conn.exec_map(r"SELECT * FROM novel WHERE uuid=:uuid", params! { "uuid" => id }, |mut row: Row| {
            println!("{:?}", &row);
            NovelRow { 
                id: row.take("id").unwrap(),  
                uuid: row.take("uuid").unwrap(), 
                category: row.take("category").unwrap(), 
                title: row.take("title").unwrap(),
                filename: row.take("filename").unwrap(), 
                synopsis: row.take("synopsis").unwrap(),
                page_count: row.take("page_count").unwrap(),
                word_count: row.take("word_count").unwrap(),
            }
        })
    })
    .await?;
    Ok(HttpResponse::Ok().json(res))
}

async fn get_one_page_from_db(pool: web::Data<DbPool>, info: web::Path<Info>) -> Result<impl Responder> {
    let id = info.id.to_string();

    let res = web::block(move || {
       
        let mut conn = pool.get().expect("could not connect to the db pool");

        conn.exec_map(r"SELECT * FROM novel WHERE id=:id", params! { "id" => id }, |mut row: Row| {
            NovelRow { 
                id: row.take("id").unwrap(),  
                uuid: row.take("uuid").unwrap(), 
                category: row.take("category").unwrap(), 
                title: row.take("title").unwrap(),
                filename: row.take("filename").unwrap(), 
                synopsis: row.take("synopsis").unwrap(),
                page_count: row.take("page_count").unwrap(),
                word_count: row.take("word_count").unwrap(),
            }
        })
    })
    .await
    .map( |novels| {
  
                  // GET THE TEXT FILE
                  let path = format!("./novels/{}", novels[0].uuid);
                  println!("novels, {:?}", &path);
                  let text = fs::read_to_string(path).unwrap();
      
                  // page 1 = 0 - 249
                  // page 2 = 250 - 499
                  // page 3 = 500 - 750 (250 * (page - 1) / 250 * page)
                  let start_slice = 250 * (&info.page - 1);
                  let end_slice = 250 * &info.page;

                  println!("{} {}", start_slice, end_slice);
                  let page_split: Vec<&str> = text.split(|c: char| c == ' ').collect();
                  let page = page_split[start_slice..end_slice].join(" ");

                  println!("{}", page);

                  // SLICE IT AT START END USING WORDS COUNT and default 250 words per page
                  HttpResponse::Ok().json(page)
    } )
    .map_err(|_| HttpResponse::InternalServerError())?;
    Ok(res)

}

#[inline]
fn is_whitespace(c: &u8) -> bool {
    *c == b' ' || *c == b'\t' || *c == b'\n'
}

#[inline]
fn is_not_empty(s: &&[u8]) -> bool {
    !s.is_empty()
}
async fn save_file(pool: web::Data<DbPool>, mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    let mut form_data = HashMap::new();

    while let Some(mut field) = payload.try_next().await? {
        // A multipart/form-data stream has to contain `content_disposition`
        let content_disposition = field
            .content_disposition()
            .ok_or_else(|| HttpResponse::BadRequest().finish())?;

        let name = content_disposition.get_name().unwrap().to_string();

        match name.as_str() {
            "title" | "category" | "synopsis" => {
                while let Some(chunk) = field.next().await {
                    form_data.insert(
                        name.to_string(),
                        std::str::from_utf8(&chunk?).unwrap().to_string(),
                    );
                }
            }
            "file" => {
                let filename = content_disposition.get_filename().unwrap().to_string();
                let uuid = format!("{}", uuid::Uuid::new_v4());
                let filepath = format!("./novels/{}", &uuid);
                
                form_data.insert(String::from("filename"), filename);
                form_data.insert(String::from("uuid"), uuid);

                // File::create is blocking operation, use threadpool
                let mut f = web::block(|| std::fs::File::create(filepath)).await?;

                // Field in turn is stream of *Bytes* object
                while let Some(chunk) = field.try_next().await? {
                    // filesystem operations are blocking, we have to use threadpool
                    let mut count = 0;
                    for _ in chunk.split(is_whitespace).filter(is_not_empty) {
                        count += 1;
                        continue;
                    }
                    form_data.insert(String::from("word_count"), count.to_string());
                    f = web::block(move || f.write_all(&chunk).map(|_| f)).await?;
                }

                // GET WORD COUNT and store in hashmap
                // GET PAGE COUNT and store in hashmap assuming font size 12 = 250 words on kindle
            }
            _ => println!(
                "this key is not covered in the match pattern, please add it - {}",
                name
            ),
        }
    }

    let res = web::block(move || {
        let mut conn = pool.get().expect("could not connect to the db pool");

        println!("{:?}", form_data.get("word_count").unwrap().parse::<i32>().unwrap() / 250);
        conn.exec_drop(
            r"INSERT INTO novel (uuid, title, category, filename, synopsis, word_count, page_count) VALUES (:uuid, :title, :category, :filename, :synopsis, :word_count, :page_count)",
            params! {
                "uuid" => form_data.get("uuid").unwrap(),
                "title" => form_data.get("title").unwrap(),
                "category" => form_data.get("category").unwrap(),
                "filename" => form_data.get("filename").unwrap(),
                "synopsis" => form_data.get("synopsis").unwrap(),
                "word_count" => form_data.get("word_count").unwrap().parse::<i32>().unwrap(),
                "page_count" => form_data.get("word_count").unwrap().parse::<i32>().unwrap() / 250
            },
        )
        .unwrap();

        conn.exec_map(r"SELECT * FROM novel WHERE uuid=:uuid", params! { "uuid" => form_data.get("uuid").unwrap() }, |mut row: Row| {
            // TODO: use ::new to create the NovelRow
            NovelRow { 
                id: row.take("id").unwrap(),  
                uuid: row.take("uuid").unwrap(), 
                category: row.take("category").unwrap(), 
                title: row.take("title").unwrap(),
                filename: row.take("filename").unwrap(), 
                synopsis: row.take("synopsis").unwrap(),
                page_count: row.take("page_count").unwrap(),
                word_count: row.take("word_count").unwrap(),
            }
        })
    })
    .await
    .map(|novel| HttpResponse::Ok().json(novel))
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
            page_count MEDIUMINT UNSIGNED NOT NULL DEFAULT 0,
            word_count INT UNSIGNED NOT NULL DEFAULT 0,
            view_count MEDIUMINT NOT NULL DEFAULT 0,
            like_count MEDIUMINT NOT NULL DEFAULT 0,
            read_time MEDIUMINT NOT NULL DEFAULT 0,
            synopsis TEXT,
            published_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        )",
    )
    .expect("could not create the table");

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(middleware::Logger::default())
            .wrap(cors)
            .data(pool.clone())
            .service(
                web::scope("/api")
                    .route("/", web::get().to(get_all))
                    .route("/meta", web::get().to(get_all_meta))
                    .route("/{id}", web::get().to(get_one))
                    .route("/v1/{id}/page/{page}", web::get().to(get_one_page))
                    .route("/{id}/page/{page}", web::get().to(get_one_page_from_db))
                    .route("/admin/upload", web::post().to(save_file)),
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
