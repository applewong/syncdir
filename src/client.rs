use core::panic;
use std::net::TcpStream;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::common::*;
use crate::fileinfo::*;

fn do_request(
    request: &Request,
    client: &mut TcpStream,
) -> Result<Response, Box<dyn std::error::Error>> {
    let frame = Frame::from_request(&request);
    frame.write(client)?;
    let frame = Frame::read(client)?;
    let response = frame.to_response().unwrap();
    Ok(response)
}

pub async fn client_main(
    server: &str,
    dir: &str,
    auth_key: &str,
    dry_run: bool,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if dry_run {
        println!("dry run");
    }
    if verbose {
        println!("sync {} from {}", dir, server);
    }
    let total_clock = Instant::now();

    let mut client = TcpStream::connect(server)?;
    //auth request
    let request = Request::Auth(auth_key.to_string());
    let response = do_request(&request, &mut client)?;
    match response {
        Response::Error(err) => {
            println!("{}", err);
            return Ok(());
        }
        _ => {}
    }

    //check self update
    let request = Request::GetFileHash("self".to_string());
    let response = do_request(&request, &mut client)?;
    match response {
        Response::FileHash(self_hash) => {
            let exe_path = std::env::current_exe()?;
            let exe_info = FileInfo::new(&exe_path)?;
            if self_hash != exe_info.hash {
                let request = Request::GetFile("self".to_string());
                let response = do_request(&request, &mut client)?;
                match response {
                    Response::File(content) => {
                        if verbose {
                            println!("updating self");
                        }
                        //move old self to .bak
                        let current_time =
                            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                        let self_name = exe_path.file_name().unwrap().to_str().unwrap().to_string();
                        let mut bak_path = exe_path.clone();
                        bak_path.set_file_name(format!("{}.{}.bak", self_name, current_time));
                        std::fs::rename(&exe_path, &bak_path)?;
                        write_compressed_file(exe_path.as_path(), content.as_slice()).await?;

                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                        let mut cmd = Command::new(exe_path);
                        cmd.arg("sync").arg("-s").arg(server).arg("-d").arg(dir);
                        if verbose {
                            cmd.arg("-v");
                        }
                        if dry_run {
                            cmd.arg("--dry-run");
                        }
                        let output = cmd.output()?;
                        println!("output of new process");
                        println!("{}", String::from_utf8_lossy(&output.stdout));
                        println!("{}", String::from_utf8_lossy(&output.stderr));
                        return Ok(());
                    }
                    _ => {}
                }

                // let file_url = format!("http://{}/file/self", server);
                // let res = client.get(file_url.as_str()).send().await?;
            }
        }
        Response::Error(err) => {
            panic!("{}", err);
        }
        _ => {
            panic!("unexpected response");
        }
    }

    //read dir_info
    let request = Request::GetDirInfo("".to_string());
    let response = do_request(&request, &mut client)?;
    match response {
        Response::DirInfo(mut base_info) => {
            let mut local_root = std::path::Path::new(dir).to_path_buf();
            if local_root.is_relative() {
                local_root = std::fs::canonicalize(&local_root)?;
            }
            base_info.set_all_file_paths(&local_root);
            let base_file_info_hashes = &base_info.flat_hashes();
            let mut total_bytes: u64 = 0;

            for (path_hash, file_info) in base_file_info_hashes {
                let local_file_info = FileInfo::new(&file_info.path);
                if local_file_info.is_err() || local_file_info.unwrap().hash != file_info.hash {
                    total_bytes += file_info.size;
                    if dry_run {
                        println!(
                            "get file: {:?} ({})",
                            file_info.path,
                            human_size(file_info.size)
                        );
                        continue;
                    }
                    let download_clock = Instant::now();
                    let request = Request::GetFile(path_hash.to_string());
                    let response = do_request(&request, &mut client)?;
                    match response {
                        Response::File(content) => {
                            if verbose {
                                println!(
                                    "download {} ({}) in {}",
                                    file_info.path.display(),
                                    human_size(file_info.size),
                                    human_duration(download_clock.elapsed()),
                                );
                            }
                            let p = file_info.path.parent().unwrap();
                            std::fs::create_dir_all(p).unwrap();
                            match write_compressed_file(&file_info.path, content.as_slice()).await {
                                Err(err) => {
                                    panic!("{:?}", err);
                                }
                                Ok(_) => {}
                            }
                        }
                        _ => {}
                    }
                }
            }

            println!(
                "total size: {:?}, done in {}.",
                human_size(total_bytes),
                human_duration(total_clock.elapsed()),
            );
        }
        _ => {}
    }
    Ok(())
}
