/*!
`s3grep` - Fast parallel grep for S3 logs

A CLI tool for searching logs and unstructured content in AWS S3 buckets.
*/

use async_compression::tokio::bufread::GzipDecoder;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, SdkConfig};
use aws_sdk_s3::config::Region;
use aws_sdk_s3::Client;
use colored::*;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use interceptors::NetworkMonitoringInterceptor;
use s3grep::line_matches;
use structopt::StructOpt;
use tokio::io::{AsyncBufReadExt, BufReader};

mod interceptors;

/// Output target for printing messages.
enum OutputTarget {
    Stdout,
    Stderr,
}

#[derive(StructOpt, Debug)]
#[structopt(name = "s3grep", about = "Fast parallel grep for S3 logs")]
struct Opt {
    /// Search pattern
    #[structopt(short, long)]
    pattern: String,

    /// S3 bucket name
    #[structopt(short, long)]
    bucket: String,

    /// S3 prefix to search in
    #[structopt(short = "z", long, default_value = "")]
    prefix: String,

    /// Number of concurrent tasks
    #[structopt(short, long, default_value = "8")]
    concurrent_tasks: usize,

    /// Case sensitive search
    #[structopt(short = "i", long)]
    case_sensitive: bool,

    /// Hide progress bar
    #[structopt(short = "q", long)]
    quiet: bool,

    /// Line numbers
    #[structopt(short = "n", long)]
    line_number: bool,
}

use anyhow::Result;

/// Entry point for the s3grep CLI application.
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        // Print a user-friendly error message and exit with code 1
        eprintln!("s3grep error: {e:?}");
        std::process::exit(1);
    }
}

pub async fn create_client_in_bucket_region_reuse_config(
    config: &SdkConfig,
    bucket_name: &str,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    // Create initial client with the default/current region
    let initial_client = Client::new(config);

    // Try to get bucket region using head_bucket
    let head_result = initial_client
        .head_bucket()
        .bucket(bucket_name)
        .send()
        .await;

    let bucket_region = match head_result {
        Ok(output) => output.bucket_region().map(str::to_owned),
        Err(err) => err
            .raw_response()
            .and_then(|res| res.headers().get("x-amz-bucket-region"))
            .map(str::to_owned),
    };

    let region = bucket_region.ok_or("Could not determine bucket region")?;

    // If the region matches the current config region, return the initial client
    if let Some(current_region) = config.region() {
        if current_region.as_ref() == region {
            return Ok(initial_client);
        }
    }

    // Create new config with the discovered region, preserving other settings
    let mut config_builder = config.to_builder();
    config_builder.set_region(Some(aws_config::Region::new(region)));
    let new_config = config_builder.build();

    // Create and return client with correct region
    Ok(Client::new(&new_config))
}

/// Main application logic for s3grep.
/// Returns Ok(()) on success, or an error on failure.
/**
    Main application logic for s3grep.

    Returns Ok(()) on success, or an error on failure.
*/
async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let opt = Opt::from_args();

    // Get or set a default region, necessary to lookup the bucket region
    // TODO: Add user opt for region: first_try("opt_region".map(Region::new))
    let region_provider = RegionProviderChain::default_provider().or_else(Region::new("us-east-1"));

    // Initialize AWS client
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let _s3_conf = aws_sdk_s3::config::Builder::from(&config)
        .interceptor(NetworkMonitoringInterceptor)
        .build();
    let client = create_client_in_bucket_region_reuse_config(&config, &opt.bucket).await?;

    // Create a progress bar that we'll update as we discover objects
    let progress = if !opt.quiet {
        let p = ProgressBar::new_spinner();
        p.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Processed {pos} files... ({per_sec} files/sec)")?,
        );
        Some(p)
    } else {
        None
    };
    let byte_progress = ProgressBar::new_spinner();
    byte_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Downloaded {total_bytes} bytes ({bytes_per_sec}/sec)")?,
    );

    let m = MultiProgress::new();
    if let Some(ref p) = progress {
        m.add(p.clone());
        m.insert_after(p, byte_progress.clone());
    }

    // Stream objects and process them concurrently
    let object_stream = list_objects_stream(&client, &opt.bucket, &opt.prefix);

    let search_stream = futures::StreamExt::map(object_stream, |obj| {
        let client = client.clone();
        let pattern = opt.pattern.clone();
        let bucket = opt.bucket.clone();
        let case_sensitive = opt.case_sensitive;
        let progress = progress.clone();
        let byte_progress = byte_progress.clone();
        let line_numbers = opt.line_number;

        async move {
            match obj {
                Ok(key) => {
                    if key.ends_with('/') {
                        print_with_target(
                            progress.as_ref(),
                            format!("{key}: Is a directory").as_str(),
                            OutputTarget::Stderr,
                        );

                        if let Some(p) = &progress {
                            p.inc(1);
                        }
                        return;
                    }

                    match search_object(
                        &client,
                        &bucket,
                        &key,
                        &pattern,
                        case_sensitive,
                        byte_progress,
                    )
                    .await
                    {
                        Ok(matches) => {
                            for (line_num, line) in matches {
                                let msg = if line_numbers {
                                    format!(
                                        "s3://{}/{}:{}:{}",
                                        bucket,
                                        key,
                                        line_num,
                                        highlight_match(&line, &pattern)
                                    )
                                } else {
                                    format!(
                                        "s3://{}/{}:{}",
                                        bucket,
                                        key,
                                        highlight_match(&line, &pattern)
                                    )
                                };
                                print_with_target(progress.as_ref(), &msg, OutputTarget::Stdout);
                            }
                        }
                        Err(e) => print_with_target(
                            progress.as_ref(),
                            format!("{key}: {e}").as_str(),
                            OutputTarget::Stderr,
                        ),
                    }
                    if let Some(p) = &progress {
                        p.inc(1);
                    }
                }
                Err(e) => {
                    print_with_target(
                        progress.as_ref(),
                        format!("Error searching objects: {e}").as_str(),
                        OutputTarget::Stderr,
                    );
                    // Print the error source chain for more detail
                    let mut source = e.source();
                    while let Some(s) = source {
                        print_with_target(
                            progress.as_ref(),
                            format!("  caused by: {s}").as_str(),
                            OutputTarget::Stderr,
                        );
                        source = s.source();
                    }
                }
            }
        }
    })
    .buffer_unordered(opt.concurrent_tasks);

    futures::StreamExt::collect::<Vec<_>>(search_stream).await;
    if let Some(p) = progress {
        // p.finish_and_clear();
        p.finish_with_message("Search complete");
        byte_progress.finish();
    }
    // progress.finish_with_message("Search complete");

    Ok(())
}

/**
    Prints a message, suspending the progress bar if present.

    # Arguments

    * `progress` - Optional progress bar to suspend.
    * `msg` - The message to print.
    * `output_fn` - Function to use for output (e.g., println! or eprintln!).
*/
fn print_message_with_progress<F>(progress: Option<&ProgressBar>, msg: &str, output_fn: F)
where
    F: Fn(&str),
{
    if let Some(p) = progress {
        p.suspend(|| output_fn(msg));
    } else {
        output_fn(msg);
    }
}

/**
    Prints a message to the specified output target, suspending the progress bar if present.

    # Arguments

    * `progress` - Optional progress bar to suspend.
    * `msg` - The message to print.
    * `target` - Output target (Stdout or Stderr).
*/
fn print_with_target(progress: Option<&ProgressBar>, msg: &str, target: OutputTarget) {
    match target {
        OutputTarget::Stdout => print_message_with_progress(progress, msg, |m| println!("{m}")),
        OutputTarget::Stderr => {
            print_message_with_progress(progress, msg, |m| eprintln!("s3grep: {m}"))
        }
    }
}

/**
    Streams S3 object keys from the specified bucket and prefix.

    # Arguments

    * `client` - AWS S3 client.
    * `bucket` - S3 bucket name.
    * `prefix` - S3 prefix to search in.

    # Returns

    A stream of object keys as `Result<String, Box<dyn std::error::Error>>`.
*/
fn list_objects_stream<'a>(
    client: &'a Client,
    bucket: &'a str,
    prefix: &'a str,
) -> impl futures::Stream<Item = Result<String, Box<dyn std::error::Error>>> + 'a {
    stream::unfold(
        (
            client.clone(),
            bucket.to_string(),
            prefix.to_string(),
            Some(String::new()),
        ),
        move |(client, bucket, prefix, continuation_token)| async move {
            // If continuation_token is None, we've finished listing
            let token = match continuation_token {
                Some(token) => token,
                None => return None,
            };

            let mut req = client
                .list_objects_v2()
                .bucket(bucket.to_owned())
                .prefix(&prefix);

            // Only set continuation token if it's not empty
            if !token.is_empty() {
                req = req.continuation_token(token);
            }

            match req.send().await {
                Ok(resp) => {
                    let objects: Vec<_> = resp
                        .contents()
                        .iter()
                        .filter_map(|obj| obj.key.clone())
                        .collect();

                    if objects.is_empty() && resp.next_continuation_token().is_none() {
                        return None;
                    }

                    let next_token = resp.next_continuation_token().map(|t| t.to_string());
                    // If we have no objects and no next token, we're done
                    if objects.is_empty() && next_token.is_none() {
                        None
                    } else {
                        Some((
                            futures::stream::iter(objects.into_iter().map(Ok)),
                            (client, bucket, prefix, next_token),
                        ))
                    }
                }
                Err(e) => {
                    eprintln!("Error listing objects: {e}");
                    let empty_vec: Vec<String> = vec![];
                    let error_stream = empty_vec
                        .into_iter()
                        .map(Ok::<String, Box<dyn std::error::Error>>);
                    Some((stream::iter(error_stream), (client, bucket, prefix, None)))
                }
            }
        },
    )
    .flatten()
}

async fn search_object(
    client: &Client,
    bucket: &str,
    key: &str,
    pattern: &str,
    case_sensitive: bool,
    byte_progress: ProgressBar,
) -> Result<Vec<(usize, String)>, Box<dyn std::error::Error>> {
    let resp = client.get_object().bucket(bucket).key(key).send().await?;

    // Add support for .gz files
    let gz_compression = key.ends_with(".gz");
    let body = resp.body.into_async_read();
    let mut reader: Box<dyn tokio::io::AsyncBufRead + Unpin> = if gz_compression {
        Box::new(BufReader::new(GzipDecoder::new(body)))
    } else {
        Box::new(BufReader::new(body))
    };

    // Binary flag
    let mut is_binary = false; //is_binary(&mut reader).await?;

    let mut matches = Vec::new();
    let mut line_buffer = Vec::new();
    let mut line_num = 0;

    loop {
        let bytes = reader.fill_buf().await?;
        if bytes.is_empty() {
            break;
        }

        // Check for NUL bytes in current buffer
        if !is_binary && bytes.contains(&0) {
            is_binary = true;
        }

        for &byte in bytes {
            if byte == b'\n' {
                line_num += 1;
                let line = String::from_utf8_lossy(&line_buffer).to_string();
                byte_progress.inc(line_buffer.len() as u64);

                if line_matches(&line, pattern, case_sensitive) {
                    if is_binary {
                        break;
                    }
                    matches.push((line_num, line));
                }
                line_buffer.clear();
            } else {
                line_buffer.push(byte);
            }
        }

        let length = bytes.len();
        reader.consume(length);
    }

    // Handle last line if it doesn't end with a newline
    if !line_buffer.is_empty() {
        line_num += 1;
        let line = String::from_utf8_lossy(&line_buffer).to_string();
        byte_progress.inc(line_buffer.len() as u64);

        if line_matches(&line, pattern, case_sensitive) {
            matches.push((line_num, line));
        }
    }

    if is_binary && !matches.is_empty() {
        print_with_target(
            Some(&byte_progress),
            format!("Binary file {key} matches").as_str(),
            OutputTarget::Stdout,
        );
        return Ok(Vec::new());
    }
    Ok(matches)
}

/**
    Highlights the first match of the pattern in the line using colored output.

    # Arguments

    * `line` - The line of text.
    * `pattern` - The pattern to highlight.

    # Returns

    The line with the first match of the pattern highlighted.
*/
fn highlight_match(line: &str, pattern: &str) -> String {
    let mut result = line.to_string();
    if let Some(start) = line.to_lowercase().find(&pattern.to_lowercase()) {
        let end = start + pattern.len();
        result.replace_range(
            start..end,
            &line[start..end].on_yellow().black().to_string(),
        );
    }
    result
}
