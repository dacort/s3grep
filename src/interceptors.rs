use aws_sdk_s3::config::interceptors::{
    BeforeSerializationInterceptorContextRef, FinalizerInterceptorContextRef,
};
use aws_smithy_runtime_api::box_error::BoxError;
use aws_smithy_runtime_api::client::interceptors::Intercept;
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponents;
use aws_smithy_types::config_bag::{ConfigBag, Storable, StoreReplace};
use std::time::Instant;

/// An interceptor for monitoring network requests and responses.
///
/// This interceptor records the start time before a request is sent and logs
/// the response size and duration after the request completes.
#[derive(Debug)]
pub(crate) struct NetworkMonitoringInterceptor;

// Wrapper type for Instant
/// Wrapper type for storing the start time of a request in the config bag.
#[derive(Debug)]
struct StartTime(Instant);

impl Storable for StartTime {
    type Storer = StoreReplace<Self>;
}

impl Intercept for NetworkMonitoringInterceptor {
    /// Returns the name of the interceptor.
    fn name(&self) -> &'static str {
        "NetworkMonitoringInterceptor"
    }

    /// Called before the request is transmitted.
    ///
    /// Stores the current time in the config bag for later duration calculation.
    fn read_before_execution(
        &self,
        _context: &BeforeSerializationInterceptorContextRef<'_>,
        cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        // let request = context.input();
        // let size_hint = request.body().content_length();
        // let estimated_size = size_hint.unwrap_or(0);
        // println!("Request size: {} bytes (estimated)", estimated_size);

        // Add timing information to the config bag
        let start_time = StartTime(Instant::now());
        cfg.interceptor_state().store_put(start_time);

        Ok(())
    }

    /// Called after the response is received and deserialized.
    ///
    /// Logs the response size and the duration of the request.
    fn read_after_execution(
        &self,
        context: &FinalizerInterceptorContextRef<'_>,
        _runtime_components: &RuntimeComponents,
        cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        if let Some(response) = context.response() {
            let size_hint = response.body().content_length();
            let estimated_size = size_hint.unwrap_or(0);
            println!("Response size: {} bytes (estimated)", estimated_size);

            // Calculate elapsed time
            if let Some(start_time) = cfg.interceptor_state().load::<StartTime>() {
                let duration = start_time.0.elapsed();
                println!("Request duration: {:?}", duration);
            }
        }

        Ok(())
    }
}
