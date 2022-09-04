use actix_web::{get, middleware::Logger, post, web, App, HttpServer, Responder};
use env_logger;
use log;

mod conf;
mod db;
mod err;
mod judge;
mod utils;

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    log::info!(target: "greet_handler", "Greeting {}", name);
    format!("Hello {name}!")
}

// DO NOT REMOVE: used in automatic testing
#[post("/internal/exit")]
#[allow(unreachable_code)]
async fn exit() -> impl Responder {
    log::info!("Shutdown as requested");
    std::process::exit(0);
    format!("Exited")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let conf = conf::Conf::parse()?;
    let server = conf.server.clone();
    println!("{:?}", conf);
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(conf.clone()))
            .wrap(Logger::default())
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(greet)
            .service(judge::post_jobs)
            .service(db::get_jobs)
            .service(db::get_job)
            .service(db::put_job)
            // DO NOT REMOVE: used in automatic testing
            .service(exit)
    })
    .bind((server.bind_address, server.bind_port))?
    .run()
    .await
}
