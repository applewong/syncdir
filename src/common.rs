// use flate2::read;
// use flate2::write;
// use flate2::Compression;
// use std::io::Read;
// use std::io::Write;

// pub fn compress_bytes(input: &[u8]) -> Vec<u8> {
//     let mut encoder = write::DeflateEncoder::new(Vec::new(), Compression::default());
//     encoder.write_all(input).unwrap();
//     encoder.finish().unwrap()
// }

// pub fn decompress_bytes(input: &[u8]) -> Vec<u8> {
//     let mut decoder = read::DeflateDecoder::new(input);
//     let mut output = Vec::new();
//     decoder.read_to_end(&mut output).unwrap();
//     output
// }

pub fn human_duration(time: std::time::Duration) -> String {
    let mut time = time.as_millis();
    let mut result = String::new();
    if time >= 1000 * 60 * 60 {
        let hours = time / (1000 * 60 * 60);
        result.push_str(&format!("{}h ", hours));
        time -= hours * 1000 * 60 * 60;
    }
    if time >= 1000 * 60 {
        let minutes = time / (1000 * 60);
        result.push_str(&format!("{}m ", minutes));
        time -= minutes * 1000 * 60;
    }
    if time >= 1000 {
        let seconds = time / 1000;
        result.push_str(&format!("{}s ", seconds));
        time -= seconds * 1000;
    }
    result.push_str(&format!("{}ms ", time));
    result
}

pub fn human_size(bytes_size: u64) -> String {
    if bytes_size < 1024 {
        return format!("{} B", bytes_size);
    } else if bytes_size < 1024 * 1024 {
        return format!("{:.2} KB", bytes_size as f64 / 1024.0);
    } else if bytes_size < 1024 * 1024 * 1024 {
        return format!("{:.2} MB", bytes_size as f64 / 1024.0 / 1024.0);
    } else {
        return format!("{:.2} GB", bytes_size as f64 / 1024.0 / 1024.0 / 1024.0);
    }
}
