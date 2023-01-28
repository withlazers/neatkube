use bytes::Bytes;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio_stream::{Stream, StreamExt};

use crate::result::Result;
use std::{path::Path, pin::Pin};

type DownloadStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + Sync>>;

pub struct Downloader {
    client: Client,
    progress: MultiProgress,
}

impl Default for Downloader {
    fn default() -> Self {
        // Reqwest setup
        let progress = MultiProgress::new();
        let client = Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "-",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .unwrap();
        Self { client, progress }
    }
}

impl Downloader {
    async fn download_to_raw(
        &self,
        stream: DownloadStream,
        file: File,
    ) -> Result<()> {
        let mut file = file;
        let mut stream = stream;

        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }
        Ok(())
    }

    pub async fn string(&self, url: &str, msg: &str) -> Result<String> {
        let mut stream = self.stream(url, msg).await?;

        let mut buf = vec![];
        while let Some(chunk) = stream.next().await {
            buf.extend_from_slice(&chunk?);
        }
        Ok(String::from_utf8(buf)?)
    }

    pub async fn file(&self, url: &str, path: &Path, msg: &str) -> Result<()> {
        let stream = self.stream(url, msg).await?;
        let temp_path = path.with_extension("part");
        fs::create_dir_all(path.parent().unwrap()).await?;
        let file = File::create(&temp_path).await?;
        match self.download_to_raw(stream, file).await {
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

    pub async fn stream(&self, url: &str, msg: &str) -> Result<DownloadStream> {
        let res = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| format!("Failed to GET from '{}'", &url))?;
        let total_size = res.content_length().unwrap_or(0);

        // Indicatif setup
        let pb = ProgressBar::new(total_size);
        let ps =ProgressStyle::default_bar()
        .template(
            r#"{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"#
            )?;
        pb.set_style(ps);
        pb.set_message(msg.to_string());

        let pb = self.progress.add(pb);

        // download chunks
        let stream = res.bytes_stream().map(move |chunk| {
            let chunk = chunk
                .map_err(|_| "Error while downloading file".to_string())?;
            pb.inc(chunk.len() as u64);
            Ok(chunk)
        });

        Ok(Box::pin(stream))
    }
}
