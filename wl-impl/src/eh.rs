use crate::libc::mcontext_t;


pub struct ExceptionContext {
    unix_context: mcontext_t,
    
}