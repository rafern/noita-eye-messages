pub type AnyError = Box<dyn std::error::Error>;
pub type AnyErrorResult<T> = Result<T, AnyError>;
pub type UnitResult = AnyErrorResult<()>;

#[macro_export]
macro_rules! main_error_wrap {
    ($block:block) => {
        if let Err(e) = (|| -> Result<(), Box<dyn std::error::Error>> {
            $block;

            Ok(())
        })() {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        };
    };
}