use async_compression::tokio::bufread::GzipDecoder;
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use colored::*;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use interceptors::NetworkMonitoringInterceptor;
use structopt::StructOpt;
use tokio::io::AsyncReadExt;

mod interceptors;


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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    // Initialize AWS client
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_conf = aws_sdk_s3::config::Builder::from(&config)
        .interceptor(NetworkMonitoringInterceptor)
        .build();
    let client = Client::new(&config);

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
        ProgressStyle::default_spinner().template("{spinner:.green} Downloaded {total_bytes} bytes ({bytes_per_sec}/sec)")?,
    );

    let m = MultiProgress::new();
    if let Some(ref p) = progress {
        m.add(p.clone());
        m.insert_after(&p, byte_progress.clone());
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
                            format!("{}: Is a directory", key).as_str(),
                            OutputTarget::Stderr,
                        );

                        if let Some(p) = &progress {
                            p.inc(1);
                        }
                        return;
                    }

                    match search_object(&client, &bucket, &key, &pattern, case_sensitive, byte_progress).await {
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
                            format!("{}: {}", key, e).as_str(),
                            OutputTarget::Stderr,
                        ),
                    }
                    if let Some(p) = &progress {
                        p.inc(1);
                    }
                }
                Err(e) => print_with_target(
                    progress.as_ref(),
                    format!("Error listing objects: {}", e).as_str(),
                    OutputTarget::Stderr,
                )
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

fn print_with_target(progress: Option<&ProgressBar>, msg: &str, target: OutputTarget) {
    match target {
        OutputTarget::Stdout => print_message_with_progress(progress, msg, |m| println!("{}", m)),
        OutputTarget::Stderr => {
            print_message_with_progress(progress, msg, |m| eprintln!("s3grep: {}", m))
        }
    }
}

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

            let mut req = client.list_objects_v2().bucket(&bucket).prefix(&prefix);

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
                Err(_) => {
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

    // TODO: Read this better
    // https://github.com/awslabs/aws-sdk-rust/blob/6b184dac648de9389afb888cc1fa54355d6d6ce2/examples/examples/s3/src/bin/get-object.rs#L41

    let mut body = Vec::new();

    // Add support for .gz files
    let gz_compression = key.ends_with(".gz");

    byte_progress.inc(resp.body.size_hint().0);

    if gz_compression {
        // If it's GZIP, use a GzDecoder to decompress
        let mut decoder = GzipDecoder::new(resp.body.into_async_read());
        decoder.read_to_end(&mut body).await?;
    } else {
        resp.body.into_async_read().read_to_end(&mut body).await?;
    }

    let content = String::from_utf8_lossy(&body);
    // This makes the transfer rate seem faster b/c we're incrementing gunziped bytes
    // byte_progress.inc(body.len() as u64);
    // println!("{}: {}", key, content);
    let matches: Vec<(usize, String)> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            // println!("Checking if {} contains {}? {}", line, pattern, line.to_lowercase().contains(&pattern.to_lowercase()));
            if case_sensitive {
                line.contains(pattern)
            } else {
                line.to_lowercase().contains(&pattern.to_lowercase())
            }
        })
        .map(|(i, line)| (i + 1, line.to_string())) // Convert to 1-based line numbers
        .collect();

    // println!("{}", matches);
    Ok(matches)
}

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
