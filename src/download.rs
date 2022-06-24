use bytes::Bytes;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio_stream::{Stream, StreamExt};

use crate::result::Result;
use std::{cmp::min, path::Path, pin::Pin};

type DownloadStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>;

async fn download_to_raw(stream: DownloadStream, file: File) -> Result<()> {
    let mut file = file;
    let mut stream = stream;

    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    Ok(())
}

pub async fn string(url: &str) -> Result<String> {
    let mut stream = stream(url).await?;

    let mut buf = vec![];
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk?);
    }
    Ok(String::from_utf8(buf)?)
}

pub async fn file(url: &str, path: &Path) -> Result<()> {
    let stream = stream(url).await?;
    let temp_path = path.with_extension("part");
    fs::create_dir_all(path.parent().unwrap()).await?;
    let file = File::create(&temp_path).await?;
    match download_to_raw(stream, file).await {
        Ok(_) => {
            fs::rename(&temp_path, path).await?;
            Ok(())
        }
        Err(e) => {
            fs::remove_file(&temp_path).await?;
            Err(e)
        }
    }
}

pub async fn stream(url: &str) -> Result<DownloadStream> {
    // Reqwest setup
    let client = Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "-",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;
    let res = client
        .get(url)
        .send()
        .await
        .or(Err(format!("Failed to GET from '{}'", &url)))?;
    let total_size = res.content_length().unwrap_or(0);

    eprintln!("Downloading {}", url);

    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("#>-"));

    // download chunks
    let mut downloaded: u64 = 0;
    let stream = res.bytes_stream().map(move |chunk| {
        let chunk = chunk.or(Err(format!("Error while downloading file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
        Ok(chunk)
    });

    Ok(Box::pin(stream))
}
