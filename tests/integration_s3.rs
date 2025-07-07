#[cfg(test)]
mod integration_s3 {
    use aws_sdk_s3::Client;
    use aws_sdk_s3::config::{Credentials, Region};
    use std::env;

    #[tokio::test]
    async fn test_list_buckets_localstack() {
        // Set up Localstack endpoint and credentials
        let endpoint = "http://localhost:4566";
        let region = Region::new("us-east-1");
        let credentials = Credentials::new(
            "test",
            "test",
            None,
            None,
            "localstack",
        );
        let config = aws_sdk_s3::config::Builder::new()
            .region(region)
            .endpoint_url(endpoint)
            .credentials_provider(credentials)
            .behavior_version_latest()
            .build();
        let client = Client::from_conf(config);

        // Attempt to list buckets (should succeed if Localstack is running)
        let resp = client.list_buckets().send().await;
        assert!(resp.is_ok(), "Failed to connect to Localstack S3");
    }
}