use actix_web::{get, middleware::Logger, post, web, App, HttpServer, Responder};
use env_logger;
use log;

mod conf;
mod db;
mod err;
mod judge;
mod utils;

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
    db::init_user();
    db::init_contest(&conf);
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(conf.clone()))
            .wrap(Logger::default())
            .service(judge::post_jobs)
            .service(db::get_jobs)
            .service(db::get_job)
            .service(db::put_job)
            .service(db::post_user)
            .service(db::get_users)
            .service(db::post_contest)
            .service(db::get_contests)
            .service(db::get_contest)
            .service(db::get_ranklist)
            // DO NOT REMOVE: used in automatic testing
            .service(exit)
    })
    .bind((server.bind_address, server.bind_port))?
    .run()
    .await
}
