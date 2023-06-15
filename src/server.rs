use crate::common::{read_file_as_compressed, Error, Frame, Request, Response};
use crate::fileinfo::*;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::collections::HashMap;
use std::path::PathBuf;
// use std::time::Duration;
use std::{net::ToSocketAddrs, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

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
                dir.files.iter().for_each(|f: &FileInfo| {
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
    update_info: tokio::sync::RwLock<UpdateInfo>,
    file_cache: tokio::sync::RwLock<HashMap<String, Arc<Vec<u8>>>>,
}

async fn handle_get_file_hash(
    app_state: Arc<AppState>,
    path_hash: &str,
) -> Result<Response, Error> {
    let update_info = app_state.update_info.read().await;
    if path_hash == "self" {
        Ok(Response::FileHash(update_info.exe_hash.clone()))
    } else {
        match update_info.file_map.get(path_hash.into()) {
            Some(file_info) => Ok(Response::FileHash(file_info.hash.clone())),
            None => Err(Error::NotFound(path_hash.into())),
        }
    }
}

async fn handle_get_file(app_state: Arc<AppState>, path_hash: &str) -> Result<Response, Error> {
    let file_cache = app_state.file_cache.read().await;
    if !file_cache.contains_key(path_hash) {
        let file_path: PathBuf;
        if path_hash == "self" {
            file_path = std::env::current_exe().unwrap();
        } else {
            let update_info = app_state.update_info.read().await;
            let v = update_info.file_map.get(path_hash.into());
            if let Some(file_info) = v {
                file_path = update_info.target_dir.join(&file_info.path);
            } else {
                return Err(Error::NotFound(path_hash.into()));
            }
        }
        drop(file_cache);
        let mut file_cache = app_state.file_cache.write().await;
        let buf = read_file_as_compressed(file_path.as_path()).await.unwrap();
        file_cache.insert(path_hash.to_string(), Arc::new(buf));
    }
    let file_cache = app_state.file_cache.read().await;
    Ok(Response::File(file_cache[path_hash].clone()))
}

pub async fn server_main(addr: &str, target_dir: &str, auth_key: &str) -> std::io::Result<()> {
    let ipv4_addrs: Vec<std::net::SocketAddr> =
        addr.to_socket_addrs()?.filter(|x| x.is_ipv4()).collect();

    if ipv4_addrs.len() == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no ipv4 address",
        ));
    }

    let app_state = Arc::new(AppState {
        update_info: RwLock::new(UpdateInfo::new(target_dir)),
        file_cache: RwLock::new(HashMap::new()),
    });

    let app_state_clone = app_state.clone();
    let target_dir_clone = target_dir.to_string();
    // let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let mut debouncer = new_debouncer(
        std::time::Duration::from_secs(10),
        None,
        move |res: notify_debouncer_mini::DebounceEventResult| {
            futures::executor::block_on(async {
                match res {
                    Ok(event) => {
                        //println!("changed: {:?}", event);
                        if let Some(_) = event.iter().find(|x| {
                            x.kind == notify_debouncer_mini::DebouncedEventKind::AnyContinuous
                        }) {
                            return;
                        }
                        let mut update_info = app_state_clone.update_info.write().await;
                        let new_update_info =
                            UpdateInfo::new(update_info.target_dir.to_str().unwrap());
                        update_info.dir_info = new_update_info.dir_info;
                        update_info.file_map = new_update_info.file_map;

                        let mut file_cache = app_state_clone.file_cache.write().await;
                        file_cache.clear();
                    }
                    Err(e) => println!("watch error: {:?}", e),
                }
            });
        },
    )
    .unwrap();

    debouncer
        .watcher()
        .watch(
            std::path::Path::new(target_dir_clone.as_str()),
            RecursiveMode::Recursive,
        )
        .unwrap();

    println!("server listening on {}", ipv4_addrs[0]);
    let listener = TcpListener::bind(ipv4_addrs[0]).await?;
    loop {
        let (mut socket, _) = listener.accept().await?;
        let app_state = app_state.clone();
        let auth_key = auth_key.to_string();
        tokio::spawn(async move {
            let (mut reader, mut writer) = socket.split();
            let mut authed: Option<bool> = Option::None;
            loop {
                let frame = Frame::read_from(&mut reader).await;
                if frame.is_err() {
                    break;
                }
                let frame = frame.unwrap();
                let request = Request::decode(&frame.data);
                match request {
                    Request::Auth(client_auth_key) => {
                        authed = Some(client_auth_key == auth_key);
                        match Frame::from_response(&Response::Auth(client_auth_key == auth_key))
                            .write_to(&mut writer)
                            .await
                        {
                            Err(_) => {
                                return;
                            }
                            _ => {}
                        }
                    }
                    Request::GetDirInfo(_) => {
                        if authed.is_none() || !authed.unwrap() {
                            println!("auth first");
                            return;
                        }
                        let response =
                            Response::DirInfo(app_state.update_info.read().await.dir_info.clone());
                        match Frame::from_response(&response).write_to(&mut writer).await {
                            Err(err) => {
                                print!("{:?}", err);
                                return;
                            }
                            _ => {}
                        }
                    }
                    Request::GetFileHash(path_hash) => {
                        if authed.is_none() || !authed.unwrap() {
                            println!("auth first");
                            return;
                        }
                        match handle_get_file_hash(app_state.clone(), &path_hash).await {
                            Ok(response) => {
                                match Frame::from_response(&response).write_to(&mut writer).await {
                                    Err(err) => {
                                        print!("{:?}", err);
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                            Err(err) => {
                                match Frame::from_response(&Response::Error(format!("{:?}", err)))
                                    .write_to(&mut writer)
                                    .await
                                {
                                    Err(err) => {
                                        println!("{:?}", err);
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Request::GetFile(path_hash) => {
                        if authed.is_none() || !authed.unwrap() {
                            println!("auth first");
                            return;
                        }
                        match handle_get_file(app_state.clone(), &path_hash).await {
                            Ok(response) => {
                                match Frame::from_response(&response).write_to(&mut writer).await {
                                    Err(err) => {
                                        print!("{:?}", err);
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                            Err(err) => {
                                match Frame::from_response(&Response::Error(format!("{:?}", err)))
                                    .write_to(&mut writer)
                                    .await
                                {
                                    Err(err) => {
                                        println!("{:?}", err);
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
