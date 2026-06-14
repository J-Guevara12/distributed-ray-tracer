use crate::Ray;

pub trait Background: Send + Sync {
    fn emit(&self, ray: &Ray);
}
