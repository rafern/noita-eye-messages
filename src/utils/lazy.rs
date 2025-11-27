/**
 * A macro which is meant to be used alongside a local Option, like a shittier
 * LazyCell, to work around issues with shared mutable references
 */
#[macro_export]
macro_rules! cached_var {
    ($option:expr, $evaluator:block) => {
        match &$option {
            Some(x) => x,
            None => {
                $option = Some($evaluator);
                $option.as_ref().unwrap()
            },
        }
    };
}