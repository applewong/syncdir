use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{fs::File, io::Write};

use reqwest::StatusCode;

use crate::common::*;
use crate::fileinfo::*;

pub async fn client_main(
    server: &str,
    dir: &str,
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

    let client: reqwest::Client = reqwest::Client::new();
    let exe_path = std::env::current_exe()?;
    let exe_info = FileInfo::new(&exe_path)?;
    let res = client
        .get(format!("http://{}/hash/self", server))
        .send()
        .await?;
    if res.status() == StatusCode::OK {
        let self_hash = res.text().await?;
        if self_hash != exe_info.hash {
            let file_url = format!("http://{}/file/self", server);
            let res = client.get(file_url.as_str()).send().await?;
            if verbose {
                println!("updating self");
            }
            //move old self to .bak
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            let self_name = exe_path.file_name().unwrap().to_str().unwrap().to_string();
            let mut bak_path = exe_path.clone();
            bak_path.set_file_name(format!("{}.{}.bak", self_name, current_time));
            std::fs::rename(&exe_path, &bak_path)?;
            let mut file = File::create(&exe_path)?;
            file.write_all(res.bytes().await?.as_ref())?;
            file.flush()?;
            drop(file);

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
    }

    let mut local_root = std::path::Path::new(dir).to_path_buf();
    if local_root.is_relative() {
        local_root = std::fs::canonicalize(&local_root)?;
    }
    let dirinfo_url = format!("http://{}/dirinfo", server);
    let res = client.get(dirinfo_url).send().await?;
    let mut base_info: DirInfo = res.json().await?;
    base_info.set_all_file_paths(&local_root);
    let mut local_info = DirInfo::new(&local_root)?;
    local_info.strip_root();
    let diffs = local_info.diff_with(&base_info);
    if dry_run {
        println!("{} files can be updated", diffs.len());
        for v in diffs {
            println!("{} ({})", v.path.display(), human_size(v.size));
        }
        return Ok(());
    }
    if verbose {
        let mut total_bytes: u64 = 0;
        for diff in &diffs {
            total_bytes += diff.size;
        }
        println!(
            "{} files ({}) to be updated.",
            diffs.len(),
            human_size(total_bytes)
        );
    }

    for diff in diffs {
        let file_url = format!("http://{}/file/{}", server, diff.path_hash);
        let download_clock = Instant::now();
        let res = client.get(file_url.as_str()).send().await?;
        if verbose {
            println!(
                "downloading {} ({}) in {}",
                diff.path.display(),
                human_size(diff.size),
                human_duration(download_clock.elapsed()),
            );
        }
        let p = diff.path.parent().unwrap();
        std::fs::create_dir_all(p).unwrap();
        let mut file = File::create(&diff.path).unwrap();
        file.write_all(res.bytes().await?.as_ref()).unwrap();
    }
    if verbose {
        println!("done in {}", human_duration(total_clock.elapsed()));
    }
    Ok(())
}
