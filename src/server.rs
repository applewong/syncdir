use std::net::ToSocketAddrs;

use crate::fileinfo::*;
use actix_files::NamedFile;
use actix_web::{get, middleware, web, App, Either, HttpResponse, HttpServer, Responder};

use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use std::sync::RwLock;
use std::time::Duration;

use std::collections::HashMap;

struct UpdateInfo {
    target_dir: std::path::PathBuf,
    dir_info: DirInfo,
    exe_hash: String,
    file_map: std::collections::HashMap<String, FileInfo>,
}

impl UpdateInfo {
    pub fn new(target_dir: &str) -> Self {
        let target_path = std::path::Path::new(target_dir).to_path_buf();
        let exe_path = std::env::current_exe().unwrap();
        let exe_info = FileInfo::new(&exe_path).unwrap();
        let mut update_info = Self {
            target_dir: target_path.clone(),
            dir_info: DirInfo::new(&target_path).unwrap(),
            exe_hash: exe_info.hash,
            file_map: HashMap::new(),
        };
        update_info.dir_info.strip_root();

        let mut file_map = HashMap::new();
        fn prepare_lookup_map(info: &UpdateInfo, map: &mut HashMap<String, FileInfo>) {
            fn traverse_dirinfo(map: &mut HashMap<String, FileInfo>, dir: &DirInfo) {
                dir.files.iter().for_each(|f| {
                    let mut file_info_cloned = f.clone();
                    file_info_cloned.path = (*dir).path.join(f.name.as_str());
                    map.insert(f.path_hash.clone(), file_info_cloned);
                });
                dir.subdirs.iter().for_each(|d| {
                    traverse_dirinfo(map, d);
                })
            }
            traverse_dirinfo(map, &info.dir_info);
        }
        prepare_lookup_map(&update_info, &mut file_map);
        update_info.file_map = file_map;
        update_info
    }
}

struct AppState {
    update_info: RwLock<UpdateInfo>,
}

#[get("/dirinfo")]
async fn get_dir_info(data: web::Data<AppState>) -> impl Responder {
    let update_info = data.update_info.read().unwrap();
    HttpResponse::Ok().json(&update_info.dir_info)
}

#[get("/hash/{path_hash}")]
async fn get_hash(data: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let path_hash = path.into_inner();
    // println!(
    //     "filemap len:{}",
    //     data.update_state.read().unwrap().file_map.len()
    // );
    let update_info = data.update_info.read().unwrap();
    if path_hash == "self" {
        return HttpResponse::Ok().body(update_info.exe_hash.clone());
    } else {
        match update_info.file_map.get(&path_hash) {
            Some(file_info) => {
                return HttpResponse::Ok().body(file_info.hash.clone());
            }
            None => {
                return HttpResponse::NotFound().body("file not found");
            }
        }
    }
}

#[get("/file/{file_hash}")]
async fn get_file(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Either<HttpResponse, Result<NamedFile, std::io::Error>> {
    let file_hash = path.into_inner();
    if file_hash == "self" {
        let exe_path = std::env::current_exe().unwrap();
        return actix_web::Either::Right(NamedFile::open_async(exe_path).await);
    } else {
        let update_info = data.update_info.read().unwrap();
        let v = update_info.file_map.get(&file_hash);
        if let Some(file_info) = v {
            let file_path = update_info.target_dir.join(&file_info.path);
            // println!("file path: {}", file_path.display());
            return actix_web::Either::Right(NamedFile::open_async(file_path).await);
        } else {
            return actix_web::Either::Left(HttpResponse::NotFound().body("file not found"));
        }
    }
}

pub async fn server_main(addr: &str, target_dir: &str) -> std::io::Result<()> {
    let ipv4_addrs: Vec<std::net::SocketAddr> =
        addr.to_socket_addrs()?.filter(|x| x.is_ipv4()).collect();

    if ipv4_addrs.len() == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no ipv4 address",
        ));
    }
    println!("server listening on {}", ipv4_addrs[0]);

    let app_state: web::Data<AppState> = web::Data::new(AppState {
        update_info: RwLock::new(UpdateInfo::new(target_dir)),
    });

    let app_state_clone = app_state.clone();
    let target_dir_clone = target_dir.to_string();
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        //setup debouncer
        // no specific tickrate, max debounce time 2 seconds
        let mut debouncer = new_debouncer(Duration::from_secs(10), None, tx).unwrap();

        debouncer
            .watcher()
            .watch(
                std::path::Path::new(target_dir_clone.as_str()),
                RecursiveMode::Recursive,
            )
            .unwrap();
        loop {
            match rx.recv() {
                Ok(_event) => {
                    // println!("event: {:?}", event);
                    let mut update_info = app_state_clone.update_info.write().unwrap();
                    let new_update_info = UpdateInfo::new(update_info.target_dir.to_str().unwrap());
                    update_info.dir_info = new_update_info.dir_info;
                    update_info.file_map = new_update_info.file_map;
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    });

    let app = move || {
        App::new()
            .wrap(middleware::Compress::default())
            .app_data(app_state.clone())
            .service(get_dir_info)
            .service(get_file)
            .service(get_hash)
    };

    HttpServer::new(app).bind(ipv4_addrs[0])?.run().await
}
