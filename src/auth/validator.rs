use crate::Channel;

pub trait Validator {
    fn validate(&self) -> crate::Result<()>;
}

impl Validator for Channel {
    fn validate(&self) -> crate::Result<()> {
        // todo
        Ok(())
    }
}
