use crate::fileinfo;
use flate2::read;
use flate2::write;
use flate2::Compression;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

pub fn compress_bytes(input: &[u8]) -> Vec<u8> {
    let mut encoder = write::DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).unwrap();
    encoder.finish().unwrap()
}

pub fn decompress_bytes(input: &[u8]) -> Vec<u8> {
    let mut decoder = read::DeflateDecoder::new(input);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).unwrap();
    output
}

use serde::{Deserialize, Serialize};

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

pub async fn read_file_as_compressed(
    file_path: &std::path::Path,
) -> Result<Vec<u8>, std::io::Error> {
    let mut f = File::open(file_path).await?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).await?;
    buf = compress_bytes(&buf);
    drop(f);
    Ok(buf)
}

pub async fn write_compressed_file(
    file_path: &std::path::Path,
    buf: &[u8],
) -> Result<(), std::io::Error> {
    let mut f = File::create(file_path).await?;
    let decmpressed = decompress_bytes(buf);
    f.write_all(decmpressed.as_slice()).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub enum Error {
    BadRequest,
    BadResponse,
    NotFound(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Request {
    Auth(String),
    GetDirInfo(String),
    GetFileHash(String),
    GetFile(String),
}

impl Request {
    pub fn encode(&self) -> Vec<u8> {
        let buf = bincode::serialize(self).unwrap();
        buf
    }

    pub fn decode(buf: &[u8]) -> Self {
        let request: Request = bincode::deserialize(buf).unwrap();
        request
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Auth(bool),
    DirInfo(fileinfo::DirInfo),
    FileHash(String),
    File(Arc<Vec<u8>>),
    Error(String),
}

impl Response {
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn decode(buf: &[u8]) -> Self {
        bincode::deserialize(&buf).unwrap()
    }
}

#[derive(Debug)]
pub struct Frame {
    pub len: u32,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn from_request(request: &Request) -> Self {
        let buf = request.encode();
        Self {
            len: buf.len() as u32,
            data: buf,
        }
    }

    pub fn from_response(response: &Response) -> Self {
        let buf = response.encode();
        Self {
            len: buf.len() as u32,
            data: buf,
        }
    }

    pub async fn read_from<'a>(
        reader: &mut tokio::net::tcp::ReadHalf<'a>,
    ) -> Result<Self, std::io::Error> {
        let mut len: [u8; 4] = [0; 4];
        // let len: u32 = reader.read_u32_le().await?;
        reader.read_exact(&mut len).await?;
        let len = u32::from_le_bytes(len);
        let mut frame = Frame {
            len,
            data: vec![0u8; len as usize],
        };

        reader.read_exact(&mut frame.data).await?;
        Ok(frame)
    }

    pub async fn write_to<'a>(
        &self,
        writer: &mut tokio::net::tcp::WriteHalf<'a>,
    ) -> Result<(), std::io::Error> {
        // self.len.to_le_bytes();
        writer.write_u32_le(self.len).await?;
        writer.write_all(self.data.as_slice()).await?;
        Ok(())
    }

    pub fn read(reader: &mut dyn std::io::Read) -> Result<Self, std::io::Error> {
        let mut len: [u8; 4] = [0; 4];
        reader.read_exact(&mut len)?;
        let len = u32::from_le_bytes(len);
        let mut frame = Frame {
            len,
            data: vec![0u8; len as usize],
        };
        reader.read_exact(&mut frame.data)?;
        Ok(frame)
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_all(&self.len.to_le_bytes())?;
        writer.write_all(self.data.as_slice())?;
        Ok(())
    }

    pub fn to_request(&self) -> Result<Request, Error> {
        let buf = self.data.as_slice();
        match bincode::deserialize(buf) {
            Ok(request) => Ok(request),
            Err(_) => Err(Error::BadRequest),
        }
    }

    pub fn to_response(&self) -> Result<Response, Error> {
        let buf = self.data.as_slice();
        match bincode::deserialize(buf) {
            Ok(response) => Ok(response),
            Err(_) => Err(Error::BadResponse),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_request() {
        let request = Request::Auth("friday".to_string());
        let bin = request.encode();
        let r2 = Request::decode(bin.as_slice());
        assert_eq!(request, r2);
    }
}
