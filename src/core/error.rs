pub type BoxDynError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub enum ApplicationError {
    Retryable(BoxDynError),
    NonRetryable(BoxDynError)
}